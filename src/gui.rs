//! GUI module using iced
//!
//! Provides a graphical launcher for TuxTalks.

use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{Element, Length, Task, Theme};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
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
}

impl Default for TuxTalksApp {
    fn default() -> Self {
        Self {
            current_tab: Tab::Home,
            status: "Ready".to_string(),
            listening: false,
            transcriptions: Vec::new(),
        }
    }
}

impl TuxTalksApp {
    pub fn new() -> (Self, Task<Message>) {
        (Self::default(), Task::none())
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
                self.listening = !self.listening;
                self.status = if self.listening {
                    "Listening...".to_string()
                } else {
                    "Stopped".to_string()
                };
            }
            Message::StartPressed => {
                self.listening = true;
                self.status = "Listening...".to_string();
            }
            Message::StopPressed => {
                self.listening = false;
                self.status = "Stopped".to_string();
            }
            Message::Transcription(text) => {
                self.transcriptions.push(text);
                if self.transcriptions.len() > 10 {
                    self.transcriptions.remove(0);
                }
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
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

    fn view_sidebar(&self) -> Element<Message> {
        let tabs = column![
            self.tab_button("🏠 Home", Tab::Home),
            self.tab_button("🎮 Games", Tab::Games),
            self.tab_button("🗣️ Speech", Tab::Speech),
            self.tab_button("⚙️ Settings", Tab::Settings),
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

    fn view_home(&self) -> Element<Message> {
        let status_text = text(&self.status).size(20);

        let listening_btn = if self.listening {
            button(text("🛑 Stop Listening"))
                .style(button::danger)
                .on_press(Message::StopPressed)
        } else {
            button(text("🎙️ Start Listening"))
                .style(button::success)
                .on_press(Message::StartPressed)
        };

        let transcription_list: Element<Message> = if self.transcriptions.is_empty() {
            text("No transcriptions yet...").into()
        } else {
            let items: Vec<Element<Message>> = self
                .transcriptions
                .iter()
                .map(|t| text(format!("• {}", t)).into())
                .collect();

            scrollable(Column::with_children(items).spacing(5)).into()
        };

        column![
            text("TuxTalks").size(32),
            text("Voice Control for Linux Gaming").size(16),
            status_text,
            listening_btn,
            text("Recent:").size(18),
            transcription_list,
        ]
        .spacing(15)
        .into()
    }

    fn view_games(&self) -> Element<Message> {
        column![
            text("Game Profiles").size(24),
            text("Configure voice commands for your games"),
            text("• Elite Dangerous"),
            text("• X4 Foundations"),
        ]
        .spacing(10)
        .into()
    }

    fn view_speech(&self) -> Element<Message> {
        column![
            text("Speech Engines").size(24),
            text("Configure ASR and TTS settings"),
            text("ASR: Vosk (Offline)"),
            text("TTS: speechd-ng"),
        ]
        .spacing(10)
        .into()
    }

    fn view_settings(&self) -> Element<Message> {
        column![
            text("Settings").size(24),
            text("General application settings"),
            text("• Audio device selection"),
            text("• Hotkeys"),
            text("• Theme"),
        ]
        .spacing(10)
        .into()
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}
