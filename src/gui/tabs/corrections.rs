use crate::gui::{Message, TuxTalksApp};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Column};
use iced::{Alignment, Element, Length};

pub fn view(app: &TuxTalksApp) -> Element<'_, Message> {
    let mut content = Column::new().spacing(20);
    content = content.push(text("Voice Corrections").size(28));
    content = content.push(
        text("Map misheard phrases to the correct command.")
            .size(16)
            .style(text::secondary),
    );

    // Input for new correction
    let add_row = row![
        column![
            text("When I say...").size(12),
            text_input("e.g. 'fire physics'", &app.new_correction_in)
                .on_input(Message::NewCorrectionInChanged)
                .width(Length::Fill),
        ]
        .width(Length::Fill)
        .spacing(5),
        column![
            text("You hear...").size(12),
            text_input("e.g. 'fire phasers'", &app.new_correction_out)
                .on_input(Message::NewCorrectionOutChanged)
                .width(Length::Fill),
        ]
        .width(Length::Fill)
        .spacing(5),
        button("Add Mapping")
            .on_press(Message::AddCorrection(
                app.new_correction_in.clone(),
                app.new_correction_out.clone()
            ))
            .style(button::primary),
    ]
    .spacing(10)
    .align_y(Alignment::End);

    content = content.push(container(add_row).padding(10).style(container::rounded_box));

    // List of corrections
    let mut list = Column::new().spacing(5);

    // Convert HashMap to Vec for sorting
    let mut corrections: Vec<_> = app.config.voice_corrections.iter().collect();
    corrections.sort_by_key(|k| k.0);

    for (input, output) in corrections {
        list = list.push(
            container(
                row![
                    text(input.clone()).width(Length::FillPortion(1)),
                    text("➜").width(Length::Fixed(30.0)),
                    text(output.clone())
                        .width(Length::FillPortion(1))
                        .style(text::success),
                    button(text("×").size(14))
                        .style(button::danger)
                        .on_press(Message::RemoveCorrection(input.clone())),
                ]
                .align_y(Alignment::Center),
            )
            .padding(10)
            .style(container::rounded_box),
        );
    }

    if app.config.voice_corrections.is_empty() {
        list = list.push(text("No corrections defined.").style(text::secondary));
    }

    content = content.push(scrollable(list).height(Length::Fill));

    content.into()
}
