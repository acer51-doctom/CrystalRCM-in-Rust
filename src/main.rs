// main.rs — CrystalRCM (Rust Edition)
// macOS RCM injector GUI built with egui + rusb.
// Async auto-update + properties file support (Tokio runtime).

use std::{
    fs,
    io::Read,
    path::Path,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use eframe::egui;
use egui::TextureHandle;
use include_dir::{include_dir, Dir};
use serde::Deserialize;

mod rcm;
mod updater;

// --- Embedded assets ---
static ASSETS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets");
const RECENT_FILES_PATH: &str = "recent_files.txt";
const PROPERTIES_PATH: &str = "assets/properties.json";
const MAX_RECENT_FILES: usize = 5;

// --- Enums ---
#[derive(Debug, PartialEq, Clone)]
enum DeviceStatus {
    Disconnected,
    Rcm,
    Normal,
    Pushing,
    Success,
    Hekate,
    Error(rcm::Error),
}

enum UsbMessage {
    StatusUpdate(DeviceStatus),
}

enum PushMessage {
    PushResult(DeviceStatus),
}

enum UpdateMessage {
    Log(String),
}

// --- App Properties ---
#[derive(Debug, Deserialize, Clone)]
struct AppProperties {
    app_name: String,
    version: String,
    description: String,
    repository: String,
    author: String,
}

impl Default for AppProperties {
    fn default() -> Self {
        Self {
            app_name: "CrystalRCM".to_string(),
            version: "0.1.0".to_string(),
            description: "A fusée gelée RCM payload injector for macOS.".to_string(),
            repository: "https://github.com/acer51-doctom/CrystalRCM-in-Rust".to_string(),
            author: "acer51-doctom".to_string(),
        }
    }
}

// --- Main ---
fn main() -> Result<(), eframe::Error> {
    let props = load_properties().unwrap_or_default();

    // Set up eframe options
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 320.0])
            .with_resizable(true)
            .with_title(format!("{} v{}", props.app_name, props.version)),
        ..Default::default()
    };

    // Tokio runtime for async updater
    let rt = tokio::runtime::Runtime::new().unwrap();

    eframe::run_native(
        &props.app_name,
        options,
        Box::new(|_cc| {
            let mut app = CrystalRcmApp::new(props);

            // Spawn async update check at startup
            let repo = app.props.repository.clone();
            let version = app.props.version.clone();
            let tx_clone = app.update_tx.clone();
            rt.spawn(async move {
                if let Err(e) = updater::check_for_updates_async(&repo, &version, tx_clone).await {
                    eprintln!("Update check failed: {}", e);
                }
            });

            Box::new(app)
        }),
    )
}

// --- GUI App ---
struct CrystalRcmApp {
    props: AppProperties,
    selected_payload: String,
    recent_payloads: Vec<String>,
    log: Vec<String>,
    device_status: DeviceStatus,
    usb_rx: Receiver<UsbMessage>,
    push_rx: Receiver<PushMessage>,
    push_tx: Option<Sender<PushMessage>>,
    update_rx: Receiver<UpdateMessage>,
    update_tx: Sender<UpdateMessage>,
    s_waiting: Option<TextureHandle>,
    s_ready: Option<TextureHandle>,
    s_hkt: Option<TextureHandle>,
    s_generic: Option<TextureHandle>,
}

impl CrystalRcmApp {
    fn new(props: AppProperties) -> Self {
        let (usb_tx, usb_rx) = mpsc::channel();
        let (push_tx, push_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();

        // USB polling thread
        thread::spawn(move || {
            let mut last_status = DeviceStatus::Disconnected;
            loop {
                let current_status = match (rcm::find_rcm_device(), rcm::find_normal_device()) {
                    (Ok(Some(_)), _) => DeviceStatus::Rcm,
                    (Ok(None), Ok(Some(_))) => DeviceStatus::Normal,
                    _ => DeviceStatus::Disconnected,
                };
                if current_status != last_status {
                    usb_tx.send(UsbMessage::StatusUpdate(current_status.clone())).unwrap();
                    last_status = current_status;
                }
                thread::sleep(Duration::from_millis(500));
            }
        });

        let recent_payloads = load_recent_files();

        Self {
            props,
            selected_payload: recent_payloads.first().cloned().unwrap_or_default(),
            recent_payloads,
            log: vec!["Welcome to CrystalRCM!".to_string()],
            device_status: DeviceStatus::Disconnected,
            usb_rx,
            push_rx,
            push_tx: Some(push_tx),
            update_rx,
            update_tx,
            s_waiting: None,
            s_ready: None,
            s_hkt: None,
            s_generic: None,
        }
    }

    // --- File dialogs ---
    fn open_payload_dialog(&mut self) {
        if let Some(path) =
            rfd::FileDialog::new().add_filter("Binary payload", &["bin"]).pick_file()
        {
            let path_str = path.to_string_lossy().to_string();
            self.log.push(format!("Set payload: {}", &path_str));
            self.selected_payload = path_str.clone();
            self.update_recent_files(path_str);
        }
    }

    // --- Push payload ---
    fn push_payload(&mut self) {
        if !Path::new(&self.selected_payload).exists() {
            self.log.push("Error: Selected payload file does not exist.".to_string());
            return;
        }

        self.device_status = DeviceStatus::Pushing;
        self.log.push("Pushing payload...".to_string());
        let payload_path = self.selected_payload.clone();
        let tx = self.push_tx.clone().unwrap();

        thread::spawn(move || {
            let result = match rcm::push_payload(&payload_path) {
                Ok(_) => {
                    if payload_path.to_lowercase().contains("hekate") {
                        DeviceStatus::Hekate
                    } else {
                        DeviceStatus::Success
                    }
                }
                Err(e) => DeviceStatus::Error(e),
            };
            tx.send(PushMessage::PushResult(result)).unwrap();
        });
    }

    // --- Handle incoming messages ---
    fn handle_usb_messages(&mut self) {
        if let Ok(UsbMessage::StatusUpdate(new_status)) = self.usb_rx.try_recv() {
            if self.device_status != DeviceStatus::Pushing {
                if new_status != self.device_status {
                    match new_status {
                        DeviceStatus::Rcm => self.log.push("RCM device connected!".to_string()),
                        DeviceStatus::Normal => {
                            self.log.push("Normal Switch connected. Please reboot to RCM.".to_string())
                        }
                        DeviceStatus::Disconnected => self.log.push("Device disconnected.".to_string()),
                        _ => {}
                    }
                }
                self.device_status = new_status;
            }
        }
    }

    fn handle_push_messages(&mut self) {
        if let Ok(PushMessage::PushResult(result)) = self.push_rx.try_recv() {
            self.device_status = result.clone();
            match result {
                DeviceStatus::Success => self.log.push("Launch complete!".to_string()),
                DeviceStatus::Hekate => self.log.push("Hekate launched successfully!".to_string()),
                DeviceStatus::Error(e) => self.log.push(format!("Error: {}", e)),
                _ => {}
            }
        }
    }

    fn handle_update_messages(&mut self) {
        while let Ok(msg) = self.update_rx.try_recv() {
            if let UpdateMessage::Log(text) = msg {
                self.log.push(text);
            }
        }
    }

    fn update_recent_files(&mut self, new_path: String) {
        self.recent_payloads.retain(|p| *p != new_path);
        self.recent_payloads.insert(0, new_path);
        self.recent_payloads.truncate(MAX_RECENT_FILES);
        save_recent_files(&self.recent_payloads);
    }

    fn load_textures_once(&mut self, ctx: &egui::Context) {
        if self.s_waiting.is_some() {
            return;
        }

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
        self.s_hkt = Some(load("s_hkt.png"));
        self.s_generic = Some(load("s_generic.png"));
    }
}

impl eframe::App for CrystalRcmApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.load_textures_once(ctx);
        self.handle_usb_messages();
        self.handle_push_messages();
        self.handle_update_messages();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Device image based on state
                let texture = match self.device_status {
                    DeviceStatus::Disconnected | DeviceStatus::Normal | DeviceStatus::Pushing => self.s_waiting.as_ref().unwrap(),
                    DeviceStatus::Rcm => self.s_ready.as_ref().unwrap(),
                    DeviceStatus::Success => self.s_generic.as_ref().unwrap(),
                    DeviceStatus::Hekate => self.s_hkt.as_ref().unwrap(),
                    DeviceStatus::Error(_) => self.s_waiting.as_ref().unwrap(),
                };
                ui.image((texture.id(), texture.size_vec2() * 0.75));

                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_source("payload_select")
                            .selected_text(
                                Path::new(&self.selected_payload)
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy(),
                            )
                            .show_ui(ui, |ui| {
                                for path in &self.recent_payloads {
                                    if ui.selectable_value(
                                        &mut self.selected_payload,
                                        path.clone(),
                                        Path::new(path)
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy(),
                                    ).clicked() {
                                        self.log.push(format!("Selected payload: {}", path));
                                    }
                                }
                            });

                        if ui.button("Payload...").clicked() {
                            self.open_payload_dialog();
                        }

                        let push_button_enabled = self.device_status == DeviceStatus::Rcm;
                        if ui.add_enabled(push_button_enabled, egui::Button::new("Push!")).clicked() {
                            self.push_payload();
                        }
                    });

                    if ui.button("Check for Updates").clicked() {
                        let repo = self.props.repository.clone();
                        let version = self.props.version.clone();
                        let tx_clone = self.update_tx.clone();
                        self.log.push("Checking for updates...".to_string());
                        // Spawn async check
                        tokio::spawn(async move {
                            if let Err(e) = updater::check_for_updates_async(&repo, &version, tx_clone).await {
                                eprintln!("Update check failed: {}", e);
                            }
                        });
                    }

                    ui.add_space(8.0);
                    egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                        let text = self.log.join("\n");
                        ui.add(
                            egui::TextEdit::multiline(&mut text.as_str())
                                .desired_width(f32::INFINITY)
                                .interactive(false),
                        );
                    });
                });
            });
        });

        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

// --- File I/O ---
fn load_recent_files() -> Vec<String> {
    fs::read_to_string(RECENT_FILES_PATH)
        .map(|content| content.lines().map(String::from).collect())
        .unwrap_or_default()
}

fn save_recent_files(files: &[String]) {
    let content = files.join("\n");
    let _ = fs::write(RECENT_FILES_PATH, content);
}

fn load_properties() -> Option<AppProperties> {
    let mut content = String::new();
    if fs::File::open(PROPERTIES_PATH)
        .and_then(|mut f| f.read_to_string(&mut content))
        .is_ok()
    {
        serde_json::from_str(&content).ok()
    } else {
        None
    }
}
