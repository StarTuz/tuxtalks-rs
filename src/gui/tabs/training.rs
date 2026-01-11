use crate::gui::{Message, TuxTalksApp};
use iced::widget::{button, column, container, text, Column};
use iced::{Alignment, Element, Length};

pub fn view(app: &TuxTalksApp) -> Element<'_, Message> {
    let mut content = Column::new().spacing(20);

    content = content.push(text("Voice Training").size(28));
    content = content.push(
        text("Improve recognition accuracy by training your voice on specific phrases.")
            .size(16)
            .style(text::secondary),
    );

    // Recording Controls based on TrainingState
    let recording_section = if app.training_state.is_recording {
        container(
            column![
                text("🔴 Recording...").size(24).style(text::danger),
                button("Stop Recording")
                    .style(button::danger)
                    .on_press(Message::ToggleTrainingRecording),
            ]
            .align_x(Alignment::Center)
            .spacing(10),
        )
        .padding(20)
        .style(container::rounded_box)
    } else {
        container(
            column![
                text("Ready to train").size(18),
                button("Start Recording")
                    .style(button::primary)
                    .on_press(Message::ToggleTrainingRecording),
            ]
            .align_x(Alignment::Center)
            .spacing(10),
        )
        .padding(20)
        .style(container::rounded_box)
    };

    content = content.push(recording_section);

    // Recommended Phrases List
    let mut phrases = Column::new().spacing(10);
    phrases = phrases.push(text("Recommended Phrases:").size(18));

    let recommended = vec![
        "computer play music",
        "computer stop playback",
        "computer switch to jriver",
        "computer switch to strawberry",
        "computer what is playing",
    ];

    for phrase in recommended {
        phrases = phrases.push(
            button(text(phrase).size(16))
                .style(button::secondary)
                .width(Length::Fill)
                .on_press(Message::SelectTrainingPhrase(phrase.to_string())),
        );
    }

    content = content.push(phrases);

    // Current phrase display
    if let Some(phrase) = &app.training_state.current_phrase {
        content = content.push(
            text(format!("Selected phrase: \"{}\"", phrase))
                .size(18)
                .style(text::primary),
        );
    }

    // Progress bar (mockup)
    if app.training_state.progress > 0.0 {
        content = content.push(text(format!(
            "Training progress: {:.0}%",
            app.training_state.progress
        )));
    }

    content.into()
}
