// main.rs - The core of the GUI application.
// It manages the application's state, handles UI rendering with egui,
// and communicates with a background thread for non-blocking USB device detection.

use std::{
    fs,
    path::Path,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
    time::Duration,
};

use eframe::egui;
// NOTE: Removed unused imports for Color32 and RichText.
use egui::TextureHandle;
use include_dir::{include_dir, Dir};

// Import the core payload launching logic from our rcm module.
mod rcm;

// Statically include the assets directory into the binary.
// This makes distribution easier as we don't need to ship a separate assets folder.
static ASSETS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets");
const RECENT_FILES_PATH: &str = "recent_files.txt";
const MAX_RECENT_FILES: usize = 5;

// Enum to represent the different states of the connected Switch device.
// NOTE: Removed `Copy` because rcm::Error does not implement it. `Clone` is sufficient.
#[derive(Debug, PartialEq, Clone)]
enum DeviceStatus {
    Disconnected,
    Rcm,
    Normal,
    Pushing,
    Success,
    Error(rcm::Error),
}

// Messages sent from the background USB polling thread to the main GUI thread.
enum UsbMessage {
    StatusUpdate(DeviceStatus),
}

// Messages sent from the GUI thread to the payload pushing thread.
enum PushMessage {
    PushResult(DeviceStatus),
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([540.0, 140.0])
            .with_resizable(false)
            .with_title("CrystalRCM (Rust Edition)"),
        ..Default::default()
    };

    eframe::run_native(
        "CrystalRCM",
        options,
        Box::new(|_cc| Box::<CrystalRcmApp>::default()),
    )
}

// The main struct holding our application's state.
struct CrystalRcmApp {
    selected_payload: String,
    recent_payloads: Vec<String>,
    log: Vec<String>,
    device_status: DeviceStatus,
    
    // Communication channels
    usb_rx: Receiver<UsbMessage>, // Receives status from USB poller
    push_rx: Receiver<PushMessage>, // Receives result from payload pusher
    push_tx: Option<Sender<PushMessage>>, // Stored to send to the pusher thread

    // UI textures
    s_waiting: Option<TextureHandle>,
    s_ready: Option<TextureHandle>,
    s_ams: Option<TextureHandle>,
    s_hkt: Option<TextureHandle>,
    s_reinx: Option<TextureHandle>,
    s_bricc: Option<TextureHandle>,
    s_lockpick: Option<TextureHandle>,
    s_generic: Option<TextureHandle>,
}

impl Default for CrystalRcmApp {
    fn default() -> Self {
        let (usb_tx, usb_rx) = mpsc::channel();
        let (push_tx, push_rx) = mpsc::channel();

        // Start the background thread for polling USB devices.
        thread::spawn(move || {
            let mut last_status = DeviceStatus::Disconnected;
            loop {
                let current_status = match (rcm::find_rcm_device(), rcm::find_normal_device()) {
                    (Ok(Some(_)), _) => DeviceStatus::Rcm,
                    (Ok(None), Ok(Some(_))) => DeviceStatus::Normal,
                    _ => DeviceStatus::Disconnected,
                };
                
                if current_status != last_status {
                    // NOTE: The value is cloned here before being moved into the send function.
                    // This leaves the original `current_status` available to be used afterwards.
                    usb_tx.send(UsbMessage::StatusUpdate(current_status.clone())).unwrap();
                    last_status = current_status;
                }
                thread::sleep(Duration::from_millis(500));
            }
        });

        let recent_payloads = load_recent_files();

        Self {
            selected_payload: recent_payloads.first().cloned().unwrap_or_default(),
            recent_payloads,
            log: vec!["Welcome to CrystalRCM!".to_string()],
            device_status: DeviceStatus::Disconnected,
            usb_rx,
            push_rx,
            push_tx: Some(push_tx),
            s_waiting: None,
            s_ready: None,
            s_ams: None,
            s_hkt: None,
            s_reinx: None,
            s_bricc: None,
            s_lockpick: None,
            s_generic: None,
        }
    }
}

impl eframe::App for CrystalRcmApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Load textures on the first frame
        self.load_textures_once(ctx);

        // Check for messages from the background threads
        self.handle_usb_messages();
        self.handle_push_messages();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                // --- Left Panel: Status Image ---
                let texture = match self.device_status {
                    DeviceStatus::Disconnected | DeviceStatus::Normal | DeviceStatus::Pushing => self.s_waiting.as_ref().unwrap(),
                    DeviceStatus::Rcm => self.s_ready.as_ref().unwrap(),
                    DeviceStatus::Success => self.s_generic.as_ref().unwrap(), // Simplified for now
                    DeviceStatus::Error(_) => self.s_waiting.as_ref().unwrap(), // Maybe an error icon later
                };
                ui.image((texture.id(), texture.size_vec2() * 0.75));

                // --- Right Panel: Controls and Log ---
                ui.vertical(|ui| {
                    // Top row: Payload selection and buttons
                    ui.horizontal(|ui| {
                        // NOTE: Prefixed with `_` to silence unused variable warning.
                        let _combo_box = egui::ComboBox::from_id_source("payload_select")
                            .selected_text(Path::new(&self.selected_payload).file_name().unwrap_or_default().to_string_lossy())
                            .show_ui(ui, |ui| {
                                for path in &self.recent_payloads {
                                    if ui.selectable_value(&mut self.selected_payload, path.clone(), Path::new(path).file_name().unwrap_or_default().to_string_lossy()).clicked() {
                                        self.log.push(format!("Selected payload: {}", path));
                                    }
                                }
                            });
                        
                        if ui.button("Payload...").clicked() {
                            self.open_payload_dialog();
                        }

                        let push_button_enabled = self.device_status == DeviceStatus::Rcm;
                        let push_button = ui.add_enabled(push_button_enabled, egui::Button::new("Push!"));
                        
                        if push_button.clicked() {
                            self.push_payload();
                        }
                    });

                    // Bottom row: Log output
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.log.join("\n"))
                                .desired_width(f32::INFINITY)
                                .desired_rows(4)
                                .interactive(false)
                        );
                    });
                });
            });
        });
        
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl CrystalRcmApp {
    /// Opens a file dialog to select a payload and updates the state.
    fn open_payload_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Binary payload", &["bin"]).pick_file() {
            let path_str = path.to_string_lossy().to_string();
            self.log.push(format!("Set payload: {}", &path_str));
            self.selected_payload = path_str.clone();
            self.update_recent_files(path_str);
        }
    }

    /// Spawns a new thread to push the selected payload.
    fn push_payload(&mut self) {
        if !Path::new(&self.selected_payload).exists() {
            self.log.push("Error: Selected payload file does not exist.".to_string());
            return;
        }

        self.device_status = DeviceStatus::Pushing;
        self.log.push("Pushing payload...".to_string());

        let payload_path = self.selected_payload.clone();
        let tx = self.push_tx.clone().unwrap(); // We always have a sender

        thread::spawn(move || {
            let result = match rcm::push_payload(&payload_path) {
                Ok(_) => DeviceStatus::Success,
                Err(e) => DeviceStatus::Error(e),
            };
            tx.send(PushMessage::PushResult(result)).unwrap();
        });
    }
    
    /// Handles incoming messages from the USB polling thread.
    fn handle_usb_messages(&mut self) {
        match self.usb_rx.try_recv() {
            Ok(UsbMessage::StatusUpdate(new_status)) => {
                // Don't override status if we're in the middle of a push
                if self.device_status != DeviceStatus::Pushing {
                    if new_status != self.device_status {
                        match new_status {
                            DeviceStatus::Rcm => self.log.push("RCM device connected!".to_string()),
                            DeviceStatus::Normal => self.log.push("Normal Switch connected. Please reboot to RCM.".to_string()),
                            DeviceStatus::Disconnected => self.log.push("Device disconnected.".to_string()),
                            _ => {}
                        }
                    }
                    self.device_status = new_status;
                }
            }
            Err(TryRecvError::Empty) => {} // No message, do nothing
            Err(TryRecvError::Disconnected) => panic!("USB Polling thread disconnected!"),
        }
    }

    /// Handles incoming messages from the payload push thread.
    fn handle_push_messages(&mut self) {
         match self.push_rx.try_recv() {
            Ok(PushMessage::PushResult(result)) => {
                self.device_status = result.clone();
                // NOTE: Pushing plain strings to the log, not RichText objects.
                 match result {
                    DeviceStatus::Success => self.log.push("Launch complete!".to_string()),
                    DeviceStatus::Error(e) => self.log.push(format!("Error: {}", e)),
                    _ => {}
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => panic!("Push thread disconnected! This should not happen."),
        }
    }

    /// Updates the list of recent files and saves it.
    fn update_recent_files(&mut self, new_path: String) {
        self.recent_payloads.retain(|p| *p != new_path);
        self.recent_payloads.insert(0, new_path);
        self.recent_payloads.truncate(MAX_RECENT_FILES);
        save_recent_files(&self.recent_payloads);
    }
    
    /// Helper to load all UI textures, but only once.
    fn load_textures_once(&mut self, ctx: &egui::Context) {
        if self.s_waiting.is_some() { return; }

        let load = |name: &str| -> TextureHandle {
            let file = ASSETS_DIR.get_file(name).unwrap();
            let image = image::load_from_memory(file.contents()).unwrap();
            let size = [image.width() as _, image.height() as _];
            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            ctx.load_texture(name, color_image, Default::default())
        };

        self.s_waiting = Some(load("s_waiting.png"));
        self.s_ready = Some(load("s_ready.png"));
        self.s_ams = Some(load("s_ams.png"));
        self.s_hkt = Some(load("s_hkt.png"));
        self.s_reinx = Some(load("s_reinx.png"));
        self.s_bricc = Some(load("s_bricc.png"));
        self.s_lockpick = Some(load("s_lockpick.png"));
        self.s_generic = Some(load("s_generic.png"));
    }
}

// --- File I/O for Recent Payloads ---

fn load_recent_files() -> Vec<String> {
    fs::read_to_string(RECENT_FILES_PATH)
        .map(|content| content.lines().map(String::from).collect())
        .unwrap_or_else(|_| Vec::new())
}

fn save_recent_files(files: &[String]) {
    let content = files.join("\n");
    let _ = fs::write(RECENT_FILES_PATH, content);
}

