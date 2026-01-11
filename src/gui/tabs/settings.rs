use crate::gui::{Message, TuxTalksApp};

use iced::widget::{button, checkbox, column, pick_list, row, text, text_input, Space};
use iced::{Element, Length};

pub fn view(app: &TuxTalksApp) -> Element<'_, Message> {
    let wake_word_section = column![
        text("Wake Word Settings").size(20),
        Space::with_height(10),
        row![
            text("Wake Word:").width(Length::Fixed(120.0)),
            text_input("e.g. 'Computer', 'Jarvis'", &app.config.wake_word)
                .on_input(Message::WakeWordChanged)
                .width(Length::Fixed(200.0)),
            Space::with_width(10),
            text("(Say this before commands)").size(14),
        ]
        .spacing(10),
    ];

    let current_vosk_model = std::path::Path::new(&app.config.vosk_model_path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let vosk_section = column![
        text("Speech Recognition (Vosk)").size(20),
        Space::with_height(10),
        row![
            text("Active Model:").width(Length::Fixed(120.0)),
            pick_list(
                &app.speech_state.available_vosk_models[..],
                Some(current_vosk_model.clone()),
                Message::SelectVoskModel
            )
            .placeholder("Select a model...")
            .width(Length::Fill),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
        row![
            button("Browse Folder").on_press(Message::BrowseVoskModel),
            button("Download Default Model").on_press(Message::DownloadModel {
                name: "vosk-model-small-en-us-0.15".to_string(),
                url: "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip"
                    .to_string(),
                is_voice: false,
            }),
            Space::with_width(Length::Fill),
            button("Delete Selected")
                .style(button::danger)
                .on_press(Message::DeleteVoskModel(current_vosk_model)),
        ]
        .spacing(10),
    ]
    .spacing(10);

    let ollama_section = column![
        text("Ollama AI Settings").size(20),
        Space::with_height(10),
        checkbox("Enable Ollama AI", app.config.ollama_enabled)
            .on_toggle(Message::OllamaEnabledToggled),
        Space::with_height(10),
        row![
            text("Ollama URL:").width(Length::Fixed(120.0)),
            text_input("http://localhost:11434", &app.config.ollama_url)
                .on_input(Message::OllamaUrlChanged)
                .width(Length::Fixed(250.0)),
        ]
        .spacing(10),
        Space::with_height(5),
        row![
            text("Model:").width(Length::Fixed(120.0)),
            text_input("llama2, mistral, etc.", &app.config.ollama_model)
                .on_input(Message::OllamaModelChanged)
                .width(Length::Fixed(200.0)),
        ]
        .spacing(10),
        Space::with_height(10),
        row![
            iced::widget::button("Check Connection").on_press(Message::OllamaHealthCheck),
            Space::with_width(10),
            match app.ollama_status {
                Some(true) => text("✅ Connected").style(|_| text::Style {
                    color: Some(iced::Color::from_rgb(0.0, 0.8, 0.0)),
                }),
                Some(false) => text("❌ Connection Failed").style(|_| text::Style {
                    color: Some(iced::Color::from_rgb(0.8, 0.0, 0.0)),
                }),
                None => text("Not checked").style(text::secondary),
            }
        ]
        .align_y(iced::Alignment::Center),
    ]
    .spacing(5);

    let player_section = column![
        text("Media Player Settings").size(20),
        Space::with_height(10),
        row![
            text("Backend:").width(Length::Fixed(120.0)),
            text_input("jriver / strawberry", &app.config.player)
                .on_input(Message::PlayerBackendChanged)
                .width(Length::Fixed(200.0)),
        ]
        .spacing(10),
        Space::with_height(5),
        text("JRiver Options:").size(16),
        row![
            text("IP:"),
            text_input("Required", &app.config.jriver_ip)
                .on_input(Message::JRiverIPChanged)
                .width(Length::Fixed(150.0)),
            text("Port:"),
            text_input("52199", &app.config.jriver_port.to_string())
                .on_input(Message::JRiverPortChanged)
                .width(Length::Fixed(80.0)),
        ]
        .spacing(10),
        row![
            text("Access Key:"),
            text_input("(Optional)", &app.config.access_key)
                .on_input(Message::JRiverAccessKeyChanged)
                .width(Length::Fixed(300.0)),
        ]
        .spacing(10),
        Space::with_height(5),
        text("Strawberry Options:").size(16),
        row![
            text("DB Path:"),
            text_input("Path to strawberry.db", &app.config.strawberry_db_path)
                .on_input(Message::StrawberryDbPathChanged)
                .width(Length::Fixed(300.0)),
        ]
        .spacing(10),
        Space::with_height(10),
        row![
            iced::widget::button("Check Player Connection").on_press(Message::PlayerHealthCheck),
            Space::with_width(10),
            match app.player_status {
                Some(true) => text("✅ Connected").style(|_| text::Style {
                    color: Some(iced::Color::from_rgb(0.0, 0.8, 0.0)),
                }),
                Some(false) => text("❌ Connection Failed").style(|_| text::Style {
                    color: Some(iced::Color::from_rgb(0.8, 0.0, 0.0)),
                }),
                None => text("Not checked").style(text::secondary),
            }
        ]
        .align_y(iced::Alignment::Center),
    ]
    .spacing(5);

    let system_info = column![
        text("System Info").size(20),
        Space::with_height(10),
        text("• Audio Input Device: Default").size(16),
        text("• Virtual Keyboard: evdev (/dev/uinput)").size(16),
        text("• UI Theme: Dark Mode").size(16),
        text("• Config Path: ~/.config/tuxtalks-rs/").size(16),
    ]
    .spacing(5);

    column![
        text("General Settings").size(28),
        Space::with_height(20),
        wake_word_section,
        Space::with_height(20),
        vosk_section,
        Space::with_height(20),
        ollama_section,
        Space::with_height(20),
        player_section,
        Space::with_height(20),
        system_info,
    ]
    .spacing(15)
    .into()
}
