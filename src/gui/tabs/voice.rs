use crate::gui::{Message, TuxTalksApp};
use iced::widget::{button, column, container, pick_list, row, text, Space};
use iced::{Alignment, Element, Length};

pub fn view(app: &TuxTalksApp) -> Element<'_, Message> {
    let mut content = column![].spacing(20);
    content = content.push(text("Voice Settings").size(24));

    // Piper Voice Selection
    let piper_section = container(column![
        text("Text-to-Speech (Piper)").size(20),
        Space::with_height(10),
        row![
            text("Active Voice:").width(Length::Fixed(120.0)),
            pick_list(
                &app.speech_state.available_piper_voices[..],
                Some(app.config.piper_voice.clone()),
                Message::SelectPiperVoice
            )
            .placeholder("Select a voice...")
            .width(Length::Fill),
            Space::with_width(10),
            button("Delete Voice")
                .style(button::danger)
                .on_press(Message::DeletePiperVoice(app.config.piper_voice.clone())),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        Space::with_height(10),
        text("Import New Voice").size(16),
        row![
            button("Download Default Voice")
                .width(Length::FillPortion(1))
                .on_press(Message::DownloadModel {
                    name: "en_GB-cori-high".to_string(),
                    url: "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_GB/cori/high/en_GB-cori-high".to_string(),
                    is_voice: true
                }),
            button("Load from File (.onnx)")
                .width(Length::FillPortion(1))
                .on_press(Message::BrowsePiperVoice),
        ]
        .spacing(10),
    ].spacing(5))
    .padding(15)
    .style(container::rounded_box)
    .width(Length::Fill);

    content = content.push(piper_section);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
