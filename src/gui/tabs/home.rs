use crate::gui::{Message, TuxTalksApp};
use iced::widget::{button, column, container, scrollable, text, Column, Space};
use iced::{Element, Length};

pub fn view(app: &TuxTalksApp) -> Element<'_, Message> {
    let status_text = text(&app.status).size(24);

    let listening_btn = if app.listening {
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

    let transcription_list: Element<Message> = if app.transcriptions.is_empty() {
        container(text("Speak a command to see it here...").style(text::secondary))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else {
        let items: Vec<Element<Message>> = app
            .transcriptions
            .iter()
            .rev()
            .map(|t| text(format!("• {}", t)).size(18).into())
            .collect();

        scrollable(Column::with_children(items).spacing(8)).into()
    };

    column![
        text("TuxTalks").size(40),
        text("Open Source AI Media Companion for Linux")
            .size(18)
            .style(text::secondary),
        Space::with_height(20),
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
