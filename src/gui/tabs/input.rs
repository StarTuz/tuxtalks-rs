use crate::gui::{Message, TuxTalksApp};

use iced::widget::{
    button, column, container, pick_list, row, scrollable, text, text_input, Column,
};
use iced::{Alignment, Element, Length};

pub fn view(app: &TuxTalksApp) -> Element<'_, Message> {
    let mut content = Column::new().spacing(20);
    content = content.push(text("Input & Hotkeys").size(28));

    let ptt_status = if app.ptt_active {
        text("🎤 PTT ACTIVE").style(text::success)
    } else {
        text("🎤 PTT Inactive (Idle)").style(text::secondary)
    };

    content = content.push(
        container(ptt_status)
            .padding(10)
            .style(container::rounded_box),
    );

    let settings = column![
        text("Push-to-Talk Settings")
            .size(20)
            .style(text::secondary),
        row![
            text("Enable PTT: "),
            button(if app.config.ptt_enabled {
                "Enabled"
            } else {
                "Disabled"
            })
            .on_press(Message::SaveConfig), // Needs TogglePtt message implementation
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        row![
            text("PTT Key: ").width(Length::Fixed(120.0)),
            text_input("e.g. KEY_LEFTCTRL", &app.config.ptt_key)
                // .on_input(Message::PttKeyChanged)
                .width(Length::Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        row![
            text("PTT Mode: "),
            pick_list(
                vec!["HOLD".to_string(), "TOGGLE".to_string()],
                Some(app.config.ptt_mode.clone()),
                |_| Message::SaveConfig // Needs PttModeChanged message implementation
            )
            .width(Length::Fixed(150.0)),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    ]
    .spacing(15);

    content = content.push(
        container(settings)
            .padding(20)
            .style(container::rounded_box),
    );

    let info = column![
        text("Global Keybindings").size(20).style(text::secondary),
        text("• Right Arrow: Next Track / Macro").size(16),
        text("• Left Arrow: Previous Track / Macro").size(16),
        text("• Home Key: Recenter (Game specific)").size(16),
    ]
    .spacing(10);

    content = content.push(container(info).padding(20).style(container::rounded_box));

    scrollable(content).into()
}
