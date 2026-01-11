use crate::gui::{Message, Tab, TuxTalksApp};
use iced::widget::{button, column, container, text, Space};
use iced::{Element, Length};

pub fn view(app: &TuxTalksApp) -> Element<'static, Message> {
    let tabs = column![
        tab_button(app, "🏠 Home", Tab::Home),
        tab_button(app, "🎮 Games", Tab::Games),
        tab_button(app, "🗣️ Speech", Tab::Speech),
        tab_button(app, "⚙️ Settings", Tab::Settings),
        tab_button(app, "🎤 Training", Tab::Training),
        tab_button(app, "📦 Packs", Tab::Packs),
        tab_button(app, "📚 Library", Tab::Player),
        tab_button(app, "⌨️ Input", Tab::Input),
        Space::with_height(Length::Fill),
        text("v0.1.0").size(12).style(text::secondary),
    ]
    .spacing(5)
    .padding(10);

    container(tabs)
        .width(Length::Fixed(180.0))
        .height(Length::Fill)
        .style(container::rounded_box)
        .into()
}

fn tab_button(app: &TuxTalksApp, label: &'static str, tab: Tab) -> Element<'static, Message> {
    let is_selected = app.current_tab == tab;

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
