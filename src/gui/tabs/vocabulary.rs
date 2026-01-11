use crate::gui::{Message, TuxTalksApp};

use iced::widget::{button, container, row, scrollable, text, text_input, Column};
use iced::{Alignment, Element, Length};

pub fn view(app: &TuxTalksApp) -> Element<'_, Message> {
    let mut content = Column::new().spacing(20);
    content = content.push(text("Custom Vocabulary").size(28));
    content = content.push(
        text("Add words that the speech engine struggles to recognize.")
            .size(16)
            .style(text::secondary),
    );

    // Input for new word
    let add_row = row![
        text_input("New vocabulary term...", &app.new_vocab_input)
            .on_input(Message::NewVocabularyInputChanged)
            .on_submit(Message::AddVocabularyTerm(app.new_vocab_input.clone()))
            .width(Length::Fill),
        button("Add Word")
            .on_press(Message::AddVocabularyTerm(app.new_vocab_input.clone()))
            .style(button::primary),
    ]
    .spacing(10);

    content = content.push(container(add_row).padding(10).style(container::rounded_box));

    // List of words
    let mut list = Column::new().spacing(5);

    // Sort for display
    let mut vocab = app.config.custom_vocabulary.clone();
    vocab.sort();

    for word in vocab {
        list = list.push(
            container(
                row![
                    text(word.clone()).width(Length::Fill),
                    button(text("×").size(14))
                        .style(button::danger)
                        .on_press(Message::RemoveVocabularyTerm(word.clone())),
                ]
                .align_y(Alignment::Center),
            )
            .padding(10)
            .style(container::rounded_box),
        );
    }

    if app.config.custom_vocabulary.is_empty() {
        list = list.push(text("No custom vocabulary defined.").style(text::secondary));
    }

    content = content.push(scrollable(list).height(Length::Fill));

    content.into()
}
