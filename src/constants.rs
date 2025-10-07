// src/constants.rs
// Contains all hardcoded constants for device identification and payload signatures.

// --- Payload Signatures (from payload_signature.py) ---
pub const FUSEE: [u8; 5] = [0xdf, 0xf0, 0x2f, 0xe3, 0x90];
pub const HEKATE_LOCKPICK: [u8; 5] = [0x08, 0x00, 0x4f, 0xe2, 0x70];
pub const BRICCMII: [u8; 5] = [0x00, 0x00, 0xa0, 0xe1, 0x00];
pub const REI: [u8; 5] = [0x08, 0x00, 0x4f, 0xe2, 0x8c];
pub const SWITCHBREW_STRING: [u8; 10] = *b"switchbrew";

// --- Device Identification (Nintendo Switch - from main.py) ---
// RCM Mode Vendor/Product ID
pub const RCM_VID: u16 = 0x0955;
pub const RCM_PID: u16 = 0x7321;

// Normal Mode Vendor/Product ID
pub const NORMAL_VID: u16 = 0x057E;
pub const NORMAL_PID: u16 = 0x2000;
