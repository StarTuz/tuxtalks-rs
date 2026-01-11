use crate::gui::{Message, TuxTalksApp};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Column, Space};
use iced::{Alignment, Element, Length};

pub fn view(app: &TuxTalksApp) -> Element<'_, Message> {
    let title = text("Macro Management").size(28);

    let active_profile = app.game_manager.get_active_profile();

    let content: Element<Message> = if let Some(profile) = active_profile {
        let mut macro_list = Column::new().spacing(25);

        if profile.macros.is_empty() {
            macro_list =
                macro_list.push(text("No macros defined for this profile.").style(text::secondary));
        } else {
            for m in &profile.macros {
                let macro_box = column![
                    row![
                        text(&m.name).size(22).width(Length::Fill),
                        button(text("🗑")).on_press(Message::RemoveMacroTrigger(
                            profile.name.clone(),
                            m.name.clone(),
                            "".to_string()
                        )) // Note: Fix remove macro logic
                    ]
                    .align_y(Alignment::Center),
                    // Triggers section
                    column![
                        text("Voice Triggers").size(16).style(text::secondary),
                        row![
                            text_input("New Trigger", &app.new_macro_trigger_input)
                                .on_input(Message::NewMacroTriggerInputChanged)
                                .width(Length::Fixed(200.0)),
                            button(text("Add")).on_press(Message::AddMacroTrigger(
                                profile.name.clone(),
                                m.name.clone(),
                                app.new_macro_trigger_input.clone()
                            )),
                        ]
                        .spacing(10),
                        row![].spacing(5).extend(m.triggers.iter().map(|t| {
                            container(
                                row![
                                    text(t),
                                    button(text("x")).on_press(Message::RemoveMacroTrigger(
                                        profile.name.clone(),
                                        m.name.clone(),
                                        t.clone()
                                    ))
                                ]
                                .spacing(5),
                            )
                            .padding(5)
                            .style(container::rounded_box)
                            .into()
                        }))
                    ]
                    .spacing(10),
                    // Steps section
                    column![
                        text("Macro Steps").size(16).style(text::secondary),
                        row![
                            text_input("Action", &app.new_macro_step_action)
                                .on_input(Message::NewMacroStepActionChanged)
                                .width(Length::Fill),
                            text_input("Delay (ms)", &app.new_macro_step_delay)
                                .on_input(Message::NewMacroStepDelayChanged)
                                .width(Length::Fixed(80.0)),
                            button(text("Add Step")).on_press(Message::AddMacroStep(
                                profile.name.clone(),
                                m.name.clone(),
                                crate::commands::MacroStep {
                                    action: "".to_string(),
                                    delay: 0,
                                    ..Default::default()
                                }
                            )),
                        ]
                        .spacing(10),
                        Column::with_children(
                            m.steps
                                .iter()
                                .enumerate()
                                .map(|(idx, step)| {
                                    row![
                                        text(format!("{}. ", idx + 1)),
                                        text(&step.action).width(Length::Fill),
                                        text(format!("{}ms", step.delay))
                                            .size(12)
                                            .style(text::secondary),
                                        button(text("x")).on_press(Message::RemoveMacroStep(
                                            profile.name.clone(),
                                            m.name.clone(),
                                            idx
                                        ))
                                    ]
                                    .spacing(10)
                                    .align_y(Alignment::Center)
                                    .into()
                                })
                                .collect::<Vec<_>>()
                        )
                    ]
                    .spacing(10),
                ]
                .spacing(15)
                .padding(15);

                macro_list = macro_list.push(
                    container(macro_box)
                        .style(container::rounded_box)
                        .padding(5),
                );
            }
        }

        let add_macro_form = row![
            text_input("New Macro Name", &app.new_macro_input)
                .on_input(Message::NewMacroInputChanged)
                .width(Length::Fixed(250.0)),
            button(text("Create Macro")).on_press(Message::AddMacro(profile.name.clone())),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        column![
            text(format!("Current Profile: {}", profile.name))
                .size(18)
                .style(text::secondary),
            Space::with_height(10),
            add_macro_form,
            Space::with_height(20),
            scrollable(macro_list)
        ]
        .spacing(10)
        .into()
    } else {
        container(
            text("No active game profile. Select one in the Games tab.").style(text::secondary),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    };

    column![title, Space::with_height(20), content]
        .padding(20)
        .into()
}
