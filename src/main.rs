// src/main.rs
// Main application logic, Iced GUI implementation, and state management.

use iced::widget::{button, column, container, row, text, vertical_space};
use iced::{
    executor, Alignment, Application, Command, Element, Length, Settings, Subscription, Theme,
};
use std::fs;
use std::io::{self, Read};
use std::path::{PathBuf};
use std::time::Duration;
use serde::{Deserialize, Serialize};

// --- Module Declarations ---
// These are necessary to link up with the constants and exploit logic.
mod constants; 
mod usb_exploits;

use constants::FUSEE; // Import payload signature for checking
use usb_exploit::{DeviceState, SwitchDevice};

// --- Application State and Logic ---

// State of the application, used for serialization of recent files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub recent_files: Vec<PathBuf>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            recent_files: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(Result<AppState, LoadError>),
    FileSelected(PathBuf),
    PayloadFileLoaded(Result<(Vec<u8>, PathBuf), String>), // Success: (data, path), Error: String
    InjectPayload,
    PollResult(DeviceState),
    Log(String),
    FatalError(String),
}

#[derive(Debug)]
pub enum LoadError {
    IoError(io::Error),
    JsonError(serde_json::Error),
}

impl From<io::Error> for LoadError {
    fn from(error: io::Error) -> Self {
        LoadError::IoError(error)
    }
}

impl From<serde_json::Error> for LoadError {
    fn from(error: serde_json::Error) -> Self {
        LoadError::JsonError(error)
    }
}

pub struct CrystalRCM {
    payload_path: String,
    recent_files: Vec<PathBuf>,
    log_output: String,
    device_state: DeviceState,
    is_loading: bool,
    payload_data: Option<Vec<u8>>,
}

impl Application for CrystalRCM {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let initial_state = Self {
            payload_path: String::new(),
            recent_files: Vec::new(),
            log_output: String::from("Waiting for device..."),
            device_state: DeviceState::Disconnected,
            is_loading: true,
            payload_data: None,
        };

        // Command to load saved state and attempt to load a default payload
        (
            initial_state,
            Command::batch(vec![
                // Load recent files list from persistent storage
                iced::storage::load(PathBuf::from("crystalrcm.json").into()), 
                // Check if 'fusee.bin' exists in the current directory
                Command::perform(Self::load_default_payload(), Message::FileSelected)
            ]),
        )
    }

    fn title(&self) -> String {
        String::from("CrystalRCM Launcher")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Loaded(Ok(state)) => {
                self.recent_files = state.recent_files;
                self.is_loading = false;
                // Attempt to load the last used payload
                if let Some(path) = self.recent_files.last().cloned() {
                    self.payload_path = path.to_string_lossy().to_string();
                    return Command::perform(Self::load_payload_file(path), |res| {
                        Message::PayloadFileLoaded(res)
                    });
                }
            }
            Message::Loaded(Err(LoadError::IoError(err))) => {
                // Ignore file not found (first run)
                if err.kind() != io::ErrorKind::NotFound {
                    self.log_output = format!("Warning: Could not load app state: {err}");
                }
                self.is_loading = false;
            }
            Message::Loaded(Err(LoadError::JsonError(err))) => {
                self.log_output = format!("Warning: Could not parse app state: {err}");
                self.is_loading = false;
            }

            Message::FileSelected(path) => {
                // If the path is empty, the file selection was cancelled.
                if path.as_os_str().is_empty() {
                    return Command::none();
                }

                self.payload_path = path.to_string_lossy().to_string();
                self.log_output = format!("Selected payload: {}", path.display());
                
                // Update recent files list and save state
                if !self.recent_files.contains(&path) {
                    self.recent_files.push(path.clone());
                    if self.recent_files.len() > 10 {
                        self.recent_files.remove(0); // Keep list size manageable
                    }
                }
                
                let state = AppState { recent_files: self.recent_files.clone() };
                let save_command = iced::storage::store(PathBuf::from("crystalrcm.json").into(), state);

                return Command::batch(vec![
                    save_command,
                    Command::perform(Self::load_payload_file(path), |res| {
                        Message::PayloadFileLoaded(res)
                    })
                ]);
            }
            
            Message::PayloadFileLoaded(Ok((data, _path))) => {
                self.payload_data = Some(data.clone());
                // Simple check for known signature (fusee)
                if data.len() >= 5 && data[..5] == FUSEE {
                    self.log_output = format!("Payload loaded and recognized as 'fusee' ({} bytes).", data.len());
                } else {
                    self.log_output = format!("Payload file loaded successfully ({} bytes). Signature unknown.", data.len());
                }
            }
            Message::PayloadFileLoaded(Err(e)) => {
                self.payload_data = None;
                self.log_output = format!("ERROR loading payload: {}", e);
            }

            Message::InjectPayload => {
                if let Some(data) = self.payload_data.clone() {
                    self.log_output = String::from("Attempting to inject payload... (Device will temporarily disappear)");
                    // Run the blocking USB exploit logic in the background
                    return Command::perform(Self::inject_rcm_payload(data), |res| match res {
                        Ok(()) => Message::Log(String::from("SUCCESS: Payload injected! Check device screen.")),
                        Err(e) => Message::FatalError(format!("INJECTION FAILED: {e}")),
                    });
                } else {
                    self.log_output = String::from("ERROR: Cannot inject, no valid payload data loaded.");
                }
            }

            Message::PollResult(new_state) => {
                // Update log only if the state has changed
                if self.device_state != new_state {
                    match new_state {
                        DeviceState::RcmMode => self.log_output = String::from("RCM device detected. Ready to push payload."),
                        DeviceState::NormalMode => self.log_output = String::from("Normal Switch device detected. RCM not available."),
                        DeviceState::Disconnected => self.log_output = String::from("Waiting for device..."),
                    }
                }
                self.device_state = new_state;
            }
            
            Message::Log(msg) => {
                self.log_output = msg;
            }
            
            Message::FatalError(msg) => {
                self.log_output = msg;
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        // Status text styling based on device state
        let state_text = match self.device_state {
            DeviceState::RcmMode => text("RCM MODE DETECTED").style(iced::theme::text::Color::Success),
            DeviceState::NormalMode => text("NORMAL MODE").style(iced::theme::text::Color::Danger),
            DeviceState::Disconnected => text("DISCONNECTED").style(iced::theme::text::Color::Default),
        };
        
        let select_file_btn = button("Select Payload (.bin)")
            .on_press(Command::perform(Self::pick_file(), Message::FileSelected));
            
        // Enable inject button only if RCM mode is active AND payload data is loaded
        let inject_enabled = self.device_state == DeviceState::RcmMode && self.payload_data.is_some();
        let inject_btn = button("🚀 PUSH PAYLOAD")
            .style(iced::theme::Button::Primary)
            .width(Length::Fill)
            .padding(10)
            .on_press_maybe(if inject_enabled {
                Some(Message::InjectPayload)
            } else {
                None
            });

        let content = column![
            text("CrystalRCM Launcher").size(24).font(iced::Font::MONOSPACE).horizontal_alignment(iced::alignment::Horizontal::Center),
            vertical_space(Length::Units(10)),
            
            // Status and Path Row
            row![
                state_text.size(18),
                text(" | Path: ").size(18),
                text(&self.payload_path).size(18),
            ].spacing(10).align_items(Alignment::Center),

            vertical_space(Length::Units(15)),

            // File Selection and Push Button Row
            row![
                select_file_btn,
                inject_btn,
            ].spacing(10).align_items(Alignment::Center).width(Length::Fill),

            vertical_space(Length::Units(10)),

            // Log Output Area
            text("DEBUG OUTPUT:").size(14),
            container(
                text(&self.log_output)
                    .font(iced::Font::MONOSPACE)
                    .size(14)
            )
            .padding(10)
            .width(Length::Fill)
            .height(Length::Units(80))
            .style(iced::theme::Container::Box),
        ]
        .spacing(10)
        .padding(20)
        .align_items(Alignment::Center);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        // Continuous device polling subscription
        if !self.is_loading {
            return Subscription::batch(vec![
                Subscription::from_recipe(DevicePolling),
            ]);
        }
        Subscription::none()
    }
}

// --- Background Task Recipes ---

// Recipe for continuous device polling (replaces threading loop)
// This is an Iced pattern to run background work at fixed intervals.
pub struct DevicePolling;

impl<H: iced::Hasher> iced::advanced::Subscription<H, Message> for DevicePolling {
    fn hash(&self, state: &mut H) {
        use std::hash::Hash;
        "device_polling".hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: iced::advanced::futures::BoxStream<iced::advanced::Event>,
    ) -> iced::advanced::futures::BoxStream<Message> {
        // Use an async stream that polls every 500ms
        Box::pin(async_std::stream::unfold(
            (),
            |_| async {
                tokio::time::sleep(Duration::from_millis(500)).await;
                
                // Call the blocking poll logic defined in the usb_exploit module
                let state = usb_exploit::SwitchDevice::poll_devices();
                Some((Message::PollResult(state), ()))
            },
        ))
    }
}

impl CrystalRCM {
    /// Asynchronously opens a native file dialog.
    async fn pick_file() -> PathBuf {
        rfd::FileDialog::new()
            .set_title("Select Payload File (.bin)")
            .add_filter("Payloads", &["bin"])
            .pick_file()
            .unwrap_or_default()
    }
    
    /// Checks for a default payload file (`fusee.bin`) next to the executable.
    async fn load_default_payload() -> PathBuf {
        let default_path = PathBuf::from("fusee.bin");
        if default_path.exists() {
            default_path
        } else {
            PathBuf::new()
        }
    }

    /// Asynchronously loads payload data from a file, executing blocking I/O on a thread.
    async fn load_payload_file(path: PathBuf) -> Result<(Vec<u8>, PathBuf), String> {
        let path_clone = path.clone();
        tokio::task::spawn_blocking(move || {
            match fs::File::open(&path_clone) {
                Ok(mut file) => {
                    let mut data = Vec::new();
                    match file.read_to_end(&mut data) {
                        Ok(_) => Ok((data, path_clone)),
                        Err(e) => Err(format!("Failed to read payload file: {e}")),
                    }
                }
                Err(e) => Err(format!("Failed to open payload file: {e}")),
            }
        }).await.unwrap_or_else(|e| Err(format!("Payload loading task failed: {e}")))
    }
    
    /// Executes the exploit by initializing the USB context and calling the injection logic.
    async fn inject_rcm_payload(payload_data: Vec<u8>) -> Result<(), String> {
        // Execute blocking USB operations in a separate blocking thread
        tokio::task::spawn_blocking(move || {
            // Re-initialize USB context for thread safety
            match rusb::Context::new() {
                Ok(ctx) => {
                    let devices = ctx.devices().map_err(|e| format!("USB enumeration error: {e}"))?;
                    
                    // Find the RCM device again
                    let device = devices.iter()
                        .filter_map(|d| d.device_descriptor().ok().map(|desc| (d, desc)))
                        .find(|(_, desc)| desc.vendor_id() == constants::RCM_VID && desc.product_id() == constants::RCM_PID)
                        .map(|(d, _)| d)
                        .ok_or_else(|| String::from("RCM device lost during injection attempt."))?;

                    let mut switch = SwitchDevice::new(device).map_err(|e| format!("Failed to open RCM device: {e}"))?;
                    
                    // The core exploit injection from usb_exploit module
                    switch.inject_payload(payload_data)
                        .map_err(|e| format!("Exploit injection failed: {e}"))?;
                        
                    Ok(())
                }
                Err(e) => Err(format!("Failed to initialize RUSB context: {e}")),
            }
        }).await.unwrap_or_else(|e| Err(format!("Injection task failed: {e}")))
    }
}

// --- Main Execution ---

pub fn main() -> iced::Result {
    // Set up window settings typical for a small, utility-focused application
    CrystalRCM::run(Settings {
        window: iced::window::Settings {
            size: (540, 300),
            resizable: false,
            ..iced::window::Settings::default()
        },
        antialiasing: true,
        ..Settings::default()
    })
}
