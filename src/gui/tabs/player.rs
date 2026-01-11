use crate::gui::{Message, TuxTalksApp};

use iced::widget::{
    button, column, container, pick_list, row, scrollable, text, text_input, Column, Space,
};
use iced::{Alignment, Element, Length};

pub fn view(app: &TuxTalksApp) -> Element<'_, Message> {
    let mut content = Column::new().spacing(20);
    content = content.push(text("Media Player Configuration").size(28));

    let backends = vec![
        "jriver".to_string(),
        "strawberry".to_string(),
        "elisa".to_string(),
        "mpris".to_string(),
    ];

    let status_text = match app.player_status {
        Some(true) => text("✅ Connected").style(text::success),
        Some(false) => text("❌ Connection Failed").style(text::danger),
        None => text("⚪ Not checked").style(text::secondary),
    };

    let backend_picker = row![
        text("Active Backend: ").size(18),
        pick_list(
            backends,
            Some(app.config.player.clone()),
            Message::PlayerBackendChanged
        )
        .width(Length::Fixed(200.0)),
        Space::with_width(20),
        status_text,
        Space::with_width(10),
        button("Check Connection")
            .on_press(Message::PlayerHealthCheck)
            .style(button::secondary),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    content = content.push(backend_picker);

    // Backend-specific settings
    let settings = match app.config.player.as_str() {
        "jriver" => column![
            text("JRiver Media Center Settings")
                .size(20)
                .style(text::secondary),
            row![
                text("IP Address: ").width(Length::Fixed(120.0)),
                text_input("e.g. localhost", &app.config.jriver_ip)
                    .on_input(Message::JRiverIPChanged)
                    .width(Length::Fill),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            row![
                text("Port: ").width(Length::Fixed(120.0)),
                text_input("e.g. 52199", &app.config.jriver_port.to_string())
                    .on_input(Message::JRiverPortChanged)
                    .width(Length::Fill),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            row![
                text("Access Key: ").width(Length::Fixed(120.0)),
                text_input("JRiver Access Key", &app.config.access_key)
                    .on_input(Message::JRiverAccessKeyChanged)
                    .width(Length::Fill),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(10),
        "strawberry" => column![
            text("Strawberry Music Player Settings")
                .size(20)
                .style(text::secondary),
            row![
                text("DB Path: ").width(Length::Fixed(120.0)),
                text_input("Path to strawberry.db", &app.config.strawberry_db_path)
                    .on_input(Message::StrawberryDbPathChanged)
                    .width(Length::Fill),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(10),
        "elisa" => column![
            text("Elisa Music Player Settings")
                .size(20)
                .style(text::secondary),
            text("Elisa settings are mostly automatic but rely on its local database.").size(14),
        ]
        .spacing(10),
        "mpris" => column![
            text("Generic MPRIS Settings")
                .size(20)
                .style(text::secondary),
            row![
                text_input("e.g. org.mpris.MediaPlayer2.vlc", &app.config.mpris_service)
                    .on_input(Message::MprisServiceChanged)
                    .width(Length::Fill),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(10),
        _ => column![text("Unknown backend selected")],
    };

    content = content.push(
        container(settings)
            .padding(20)
            .style(container::rounded_box),
    );

    // Common Library Settings (Always visible)
    let library_settings = column![
        text("Music Library").size(20),
        row![
            text("Library Path: ").width(Length::Fixed(120.0)),
            text_input("Path to scan for music", &app.config.library_path)
                .on_input(Message::LibraryPathChanged)
                .width(Length::Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        button("Scan Library Now")
            .style(button::primary)
            .on_press(Message::ScanLibrary),
    ]
    .spacing(10);

    content = content.push(
        container(library_settings)
            .padding(20)
            .style(container::rounded_box),
    );

    scrollable(content).into()
}
