//! GUI module using iced
//!
//! Provides a graphical launcher for TuxTalks.

use iced::widget::{button, column, container, row, scrollable, text, Column, pick_list, Space};
use iced::{Element, Length, Task, Theme, Subscription, Alignment};
use futures::StreamExt;
use tracing::{info, warn, debug};

use crate::games::{GameManager, GameProfile};
use crate::asr::VoskAsr;
use crate::audio;
use crate::commands::{CommandProcessor, Command};

/// Main application state
pub struct TuxTalksApp {
    /// Current view/tab
    current_tab: Tab,
    /// Status message
    status: String,
    /// Is listening active
    listening: bool,
    /// Recent transcriptions
    transcriptions: Vec<String>,
    /// Game Manager
    game_manager: GameManager,
    /// Command Processor
    processor: CommandProcessor,
    /// TTS Client
    speechd: Option<crate::speechd::SpeechdClient>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Home,
    Games,
    Speech,
    Settings,
}

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(Tab),
    ToggleListening,
    Transcription(String),
    StartPressed,
    StopPressed,
    ProfileSelected(String),
    AutoDetect,
    SpeechdConnected(crate::speechd::SpeechdClient),
    SpeechdFailed,
}

impl TuxTalksApp {
    pub fn new() -> (Self, Task<Message>) {
        let game_manager = GameManager::new().expect("Failed to init GameManager");
        let processor = CommandProcessor::new().expect("Failed to init CommandProcessor");

        let app = Self {
            current_tab: Tab::Home,
            status: "Ready".to_string(),
            listening: false,
            transcriptions: Vec::new(),
            game_manager,
            processor,
            speechd: None,
        };

        // Initialize speechd in background
        let init_task = Task::perform(crate::speechd::SpeechdClient::connect(), |res| {
            match res {
                Ok(client) => Message::SpeechdConnected(client),
                Err(_) => Message::SpeechdFailed,
            }
        });

        (app, init_task)
    }

    pub fn title(&self) -> String {
        "TuxTalks".to_string()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(tab) => {
                self.current_tab = tab;
            }
            Message::ToggleListening => {
                if self.listening {
                    return self.update(Message::StopPressed);
                } else {
                    return self.update(Message::StartPressed);
                }
            }
            Message::StartPressed => {
                self.listening = true;
                self.status = "Listening...".to_string();
                
                // Load commands from active profile if any
                if let Some(profile) = self.game_manager.get_active_profile() {
                    let commands = profile.get_processor_commands();
                    info!("🚀 Loading {} commands from profile '{}'", commands.len(), profile.name);
                    
                    // Reset and load
                    self.processor = CommandProcessor::new().unwrap();
                    for cmd in commands {
                        self.processor.add_command(cmd);
                    }
                    self.processor.set_action_map(profile.resolve_actions());
                } else {
                    self.processor.add_demo_bindings();
                }
            }
            Message::StopPressed => {
                self.listening = false;
                self.status = "Stopped".to_string();
            }
            Message::Transcription(text) => {
                if !text.is_empty() {
                    self.transcriptions.push(text.clone());
                    if self.transcriptions.len() > 10 {
                        self.transcriptions.remove(0);
                    }

                    // Process command
                    if let Some(cmd_name) = self.processor.process(&text) {
                        self.status = format!("Executed: {}", cmd_name);
                        
                        // TTS Feedback
                        if let Some(speechd) = &self.speechd {
                            let text_to_speak = format!("Executing {}", cmd_name);
                            return Task::perform(msg_speak(speechd.clone(), text_to_speak), |_| Message::AutoDetect); // Dummy message for task completion
                        }
                    }
                }
            }
            Message::ProfileSelected(name) => {
                if let Some(idx) = self.game_manager.profiles.iter().position(|p| p.name == name) {
                    self.game_manager.active_profile_index = Some(idx);
                    info!("🎯 Profile selected: {}", name);
                    self.game_manager.save_profiles().ok();
                }
            }
            Message::AutoDetect => {
                if let Some(idx) = self.game_manager.detect_active_profile() {
                    let name = &self.game_manager.profiles[idx].name;
                    info!("🤖 Auto-detected profile: {}", name);
                    self.status = format!("Detected: {}", name);
                }
            }
            Message::SpeechdConnected(client) => {
                info!("🔊 Speechd connected");
                self.speechd = Some(client);
            }
            Message::SpeechdFailed => {
                warn!("🔇 Speechd failed to connect");
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = Vec::new();

        // ASR Subscription
        if self.listening {
            subscriptions.push(Subscription::run(|| {
                futures::stream::unfold((), |_| async move {
                    // Start capture
                    let mut audio_rx = match audio::start_capture(None) {
                        Ok(rx) => rx,
                        Err(e) => {
                            warn!("Failed to start audio capture: {}", e);
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            return Some((Message::Transcription("Error: No Audio".into()), ()));
                        }
                    };

                    // Start ASR
                    let mut asr = match VoskAsr::new() {
                        Ok(asr) => asr,
                        Err(e) => {
                            warn!("Failed to start ASR: {}", e);
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            return Some((Message::Transcription("Error: No ASR".into()), ()));
                        }
                    };

                    loop {
                        if let Some(samples) = audio_rx.recv().await {
                            match asr.process(&samples) {
                                Ok(Some(text)) => {
                                    return Some((Message::Transcription(text), ()));
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    warn!("ASR error: {}", e);
                                    break;
                                }
                            }
                        } else {
                            break;
                        }
                    }

                    Some((Message::Transcription("ASR Restarting...".into()), ()))
                })
            }));
        }

        // Periodic Auto-Detection Subscription
        subscriptions.push(iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::AutoDetect));

        Subscription::batch(subscriptions)
    }

    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = self.view_sidebar();
        let content = match self.current_tab {
            Tab::Home => self.view_home(),
            Tab::Games => self.view_games(),
            Tab::Speech => self.view_speech(),
            Tab::Settings => self.view_settings(),
        };

        let main_content = row![
            sidebar,
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(20)
        ];

        container(main_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        let tabs = column![
            self.tab_button("🏠 Home", Tab::Home),
            self.tab_button("🎮 Games", Tab::Games),
            self.tab_button("🗣️ Speech", Tab::Speech),
            self.tab_button("⚙️ Settings", Tab::Settings),
            Space::with_height(Length::Fill),
            text("v0.1.0").size(12).style(text::secondary),
        ]
        .spacing(5)
        .padding(10);

        container(tabs)
            .width(Length::Fixed(150.0))
            .height(Length::Fill)
            .style(container::rounded_box)
            .into()
    }

    fn tab_button(&self, label: &'static str, tab: Tab) -> Element<'static, Message> {
        let is_selected = self.current_tab == tab;

        button(text(label))
            .width(Length::Fill)
            .padding(10)
            .style(if is_selected {
                button::primary
            } else {
                button::secondary
            })
            .on_press(Message::TabSelected(tab))
            .into()
    }

    fn view_home(&self) -> Element<'_, Message> {
        let status_text = text(&self.status).size(24);

        let listening_btn = if self.listening {
            button(text("🛑 Stop Listening"))
                .padding(12)
                .style(button::danger)
                .on_press(Message::StopPressed)
        } else {
            button(text("🎙️ Start Listening"))
                .padding(12)
                .style(button::success)
                .on_press(Message::StartPressed)
        };

        let transcription_list: Element<Message> = if self.transcriptions.is_empty() {
            container(text("Speak a command to see it here...").style(text::secondary))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        } else {
            let items: Vec<Element<Message>> = self
                .transcriptions
                .iter()
                .rev()
                .map(|t| text(format!("• {}", t)).size(18).into())
                .collect();

            scrollable(Column::with_children(items).spacing(8)).into()
        };

        // Profile picker
        let profile_names: Vec<String> = self.game_manager.profiles.iter().map(|p| p.name.clone()).collect();
        let selected_profile = self.game_manager.get_active_profile().map(|p| p.name.clone());

        let profile_picker = row![
            text("Active Profile: ").size(18),
            pick_list(
                profile_names,
                selected_profile,
                Message::ProfileSelected
            ).placeholder("Select a game...")
        ].spacing(10).align_y(Alignment::Center);

        column![
            text("TuxTalks").size(40),
            text("Voice Control for Linux Gaming").size(18).style(text::secondary),
            Space::with_height(20),
            profile_picker,
            Space::with_height(10),
            status_text,
            listening_btn,
            Space::with_height(20),
            text("Recent Transcriptions:").size(20),
            container(transcription_list)
                .padding(10)
                .style(container::rounded_box)
                .height(Length::Fill)
        ]
        .spacing(15)
        .into()
    }

    fn view_games(&self) -> Element<'_, Message> {
        let mut content = Column::new().spacing(15);
        content = content.push(text("Game Profiles").size(28));

        // Active Profile Detail
        if let Some(profile) = self.game_manager.get_active_profile() {
            content = content.push(
                container(
                    column![
                        text(format!("Selected: {}", profile.name)).size(22),
                        text(format!("Type: {:?}", profile.game_type)).size(14).style(text::secondary),
                        Space::with_height(10),
                        text("Resolved Action Mapping:").size(18),
                        self.view_resolved_actions(profile),
                    ].spacing(5).padding(15)
                ).style(container::rounded_box)
            );
        }

        content = content.push(Space::with_height(20));
        content = content.push(text("All Available Profiles:").size(20));

        for profile in &self.game_manager.profiles {
            let info = column![
                text(&profile.name).size(20),
                text(format!("{} triggers | {} macros", profile.voice_commands.len(), profile.macros.len()))
                    .size(14).style(text::secondary),
            ];

            content = content.push(
                container(
                    row![
                        info.width(Length::Fill),
                        button("Activate").on_press(Message::ProfileSelected(profile.name.clone())),
                    ].align_y(Alignment::Center).padding(15)
                ).style(container::rounded_box)
            );
        }

        scrollable(content).into()
    }

    fn view_resolved_actions(&self, profile: &GameProfile) -> Element<'_, Message> {
        let actions = profile.resolve_actions();
        if actions.is_empty() {
            return text("No actions resolved. Are your bindings loaded?").style(text::danger).into();
        }

        let mut list = Column::new().spacing(5);
        let mut keys: Vec<String> = actions.keys().cloned().collect();
        keys.sort();

        for name in keys {
            if let Some(binding) = actions.get(&name) {
                let key_str = binding.primary_key.as_deref().unwrap_or("None");
                let mods = if binding.modifiers.is_empty() {
                    "".to_string()
                } else {
                    format!(" (+ {:?})", binding.modifiers)
                };
                
                list = list.push(
                    row![
                        text(name.to_string()).width(Length::Fixed(150.0)),
                        text("👉").width(Length::Fixed(30.0)),
                        text(format!("{}{}", key_str, mods)).style(text::success),
                    ]
                );
            }
        }

        container(list).padding(10).into()
    }

    fn view_speech(&self) -> Element<'_, Message> {
        column![
            text("Speech Engines").size(28),
            Space::with_height(10),
            text("ASR Configuration:").size(20),
            text("• Engine: Vosk (Offline)").size(16),
            text("• Model: vosk-model-small-en-us").size(16),
            Space::with_height(20),
            text("TTS Configuration:").size(20),
            text("• Backend: speechd-ng via D-Bus").size(16),
        ]
        .spacing(10)
        .into()
    }

    fn view_settings(&self) -> Element<'_, Message> {
        column![
            text("Settings").size(28),
            Space::with_height(10),
            text("• Audio Input Device: Default").size(18),
            text("• Virtual Keyboard: evdev (/dev/uinput)").size(18),
            text("• UI Theme: Dark Mode").size(18),
        ]
        .spacing(15)
        .into()
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}

async fn msg_speak(client: crate::speechd::SpeechdClient, text: String) {
    let _ = client.speak(&text).await;
}
