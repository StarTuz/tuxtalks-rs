use crate::games::GameProfile;
use crate::gui::{Message, TuxTalksApp};
use iced::widget::{
    button, column, container, pick_list, row, scrollable, text, text_input, Column, Space,
};
use iced::{Alignment, Element, Length};

pub fn view(app: &TuxTalksApp) -> Element<'_, Message> {
    if let Some(idx) = app.editing_profile_idx {
        if let Some(profile) = app.game_manager.profiles.get(idx) {
            return view_profile_editor(app, profile);
        }
    }

    let mut content = Column::new().spacing(15);

    // Profile picker
    let profile_names: Vec<String> = app
        .game_manager
        .profiles
        .iter()
        .map(|p| p.name.clone())
        .collect();
    let selected_profile = app
        .game_manager
        .get_active_profile()
        .map(|p| p.name.clone());

    let profile_picker = row![
        text("Active Profile: ").size(18),
        pick_list(profile_names, selected_profile, Message::ProfileSelected)
            .placeholder("Select a game...")
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let header = row![
        text("Game Profiles").size(28).width(Length::Fill),
        profile_picker,
        button(row![text("➕ Add Game")].spacing(5))
            .on_press(Message::OpenAddGameWizard)
            .padding(10)
            .style(button::primary),
    ]
    .spacing(20)
    .align_y(Alignment::Center);

    content = content.push(header);

    // Active Profile Detail
    if let Some(profile) = app.game_manager.get_active_profile() {
        content = content.push(
            container(
                column![
                    text(format!("Selected: {}", profile.name)).size(22),
                    text(format!("Type: {:?}", profile.game_type))
                        .size(14)
                        .style(text::secondary),
                    Space::with_height(10),
                    text("Resolved Action Mapping:").size(18),
                    view_resolved_actions(profile),
                ]
                .spacing(5)
                .padding(15),
            )
            .style(container::rounded_box),
        );
    }

    content = content.push(Space::with_height(20));
    content = content.push(text("All Available Profiles:").size(20));

    for profile in &app.game_manager.profiles {
        let info = column![
            text(&profile.name).size(20),
            text(format!(
                "{} triggers | {} macros",
                profile.voice_commands.len(),
                profile.macros.len()
            ))
            .size(14)
            .style(text::secondary),
        ];

        content = content.push(
            container(
                row![
                    info.width(Length::Fill),
                    button("Edit").on_press(Message::EditProfile(
                        app.game_manager
                            .profiles
                            .iter()
                            .position(|p| p.name == profile.name)
                            .unwrap()
                    )),
                    Space::with_width(10),
                    button("Activate").on_press(Message::ProfileSelected(profile.name.clone())),
                ]
                .align_y(Alignment::Center)
                .padding(15),
            )
            .style(container::rounded_box),
        );
    }

    scrollable(content).into()
}

fn view_profile_editor<'a>(app: &'a TuxTalksApp, profile: &'a GameProfile) -> Element<'a, Message> {
    let mut content = Column::new().spacing(15);

    let header = row![
        text(format!("Editing: {}", profile.name))
            .size(28)
            .width(Length::Fill),
        button("Back to List").on_press(Message::CloseEditor),
    ]
    .align_y(Alignment::Center);

    let path_str = profile
        .bindings_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let path_editor = row![
        text("Bindings Path: ").size(16).width(Length::Fixed(120.0)),
        text_input("Path to XML file...", &path_str)
            .on_input(Message::BindingsPathChanged)
            .width(Length::Fill),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    content = content.push(header);
    content = content.push(path_editor);
    content = content.push(Space::with_height(10));
    content = content.push(
        text("Voice Commands & Triggers")
            .size(20)
            .style(text::secondary),
    );

    let mut commands_list = Column::new().spacing(20);

    // Sort friendly names for consistent UI
    let mut friendly_names: Vec<_> = profile.voice_commands.keys().collect();
    friendly_names.sort();

    for friendly in friendly_names {
        let triggers = profile.voice_commands.get(friendly).unwrap();

        let mut triggers_row = row![].spacing(10).align_y(Alignment::Center);
        for trigger in triggers {
            triggers_row = triggers_row.push(
                container(
                    row![
                        text(trigger.to_string()),
                        button(text("×").size(12))
                            .style(button::danger)
                            .on_press(Message::RemoveTrigger(friendly.clone(), trigger.clone())),
                    ]
                    .spacing(5)
                    .align_y(Alignment::Center),
                )
                .padding(5)
                .style(container::rounded_box),
            );
        }

        let add_trigger_input = row![
            text_input("New trigger...", &app.new_trigger_input)
                .on_input(Message::NewTriggerInputChanged)
                .on_submit(Message::AddTrigger(
                    friendly.clone(),
                    app.new_trigger_input.clone()
                ))
                .width(Length::Fixed(150.0)),
            button("Add").on_press(Message::AddTrigger(
                friendly.clone(),
                app.new_trigger_input.clone()
            )),
        ]
        .spacing(5);

        commands_list = commands_list.push(
            column![
                row![
                    text(friendly.to_string())
                        .size(20)
                        .style(text::primary)
                        .width(Length::Fill),
                    button(text("Delete Command").size(12))
                        .style(button::danger)
                        .on_press(Message::RemoveFriendlyName(friendly.clone())),
                ]
                .align_y(Alignment::Center),
                triggers_row,
                add_trigger_input,
                Space::with_height(10),
            ]
            .spacing(8),
        );
    }

    content = content.push(scrollable(commands_list));

    // Add new command section at the bottom
    let add_command_row = row![
        text_input(
            "New command name (e.g. 'Shields')",
            &app.new_friendly_name_input
        )
        .on_input(Message::NewFriendlyNameInputChanged)
        .on_submit(Message::AddFriendlyName(
            app.new_friendly_name_input.clone()
        ))
        .width(Length::Fill),
        button("Add Command Entry").on_press(Message::AddFriendlyName(
            app.new_friendly_name_input.clone()
        )),
    ]
    .spacing(10)
    .padding(10);

    content = content.push(container(add_command_row).style(container::rounded_box));

    content = content.push(Space::with_height(20));
    content = content.push(text("Macros & Triggers").size(20).style(text::secondary));

    let mut macros_list = Column::new().spacing(20);
    for m in &profile.macros {
        let mut triggers_row = row![].spacing(10).align_y(Alignment::Center);
        for trigger in &m.triggers {
            triggers_row = triggers_row.push(
                container(
                    row![
                        text(trigger.to_string()),
                        button(text("×").size(12)).style(button::danger).on_press(
                            Message::RemoveMacroTrigger(
                                profile.name.clone(),
                                m.name.clone(),
                                trigger.clone()
                            )
                        ),
                    ]
                    .spacing(5)
                    .align_y(Alignment::Center),
                )
                .padding(5)
                .style(container::rounded_box),
            );
        }

        let add_trigger_input = row![
            text_input("New trigger...", &app.new_trigger_input)
                .on_input(Message::NewTriggerInputChanged)
                .on_submit(Message::AddMacroTrigger(
                    profile.name.clone(),
                    m.name.clone(),
                    app.new_trigger_input.clone()
                ))
                .width(Length::Fixed(150.0)),
            button("Add").on_press(Message::AddMacroTrigger(
                profile.name.clone(),
                m.name.clone(),
                app.new_trigger_input.clone()
            )),
        ]
        .spacing(5);

        macros_list = macros_list.push(
            column![
                text(format!("{} ({} steps)", m.name, m.steps.len()))
                    .size(18)
                    .style(text::primary),
                triggers_row,
                add_trigger_input,
            ]
            .spacing(8),
        );
    }

    content = content.push(scrollable(macros_list));

    let add_macro_row = row![
        text_input("New macro name", &app.new_macro_input)
            .on_input(Message::NewMacroInputChanged)
            .on_submit(Message::AddMacro(app.new_macro_input.clone()))
            .width(Length::Fill),
        button("Add Macro").on_press(Message::AddMacro(app.new_macro_input.clone())),
    ]
    .spacing(10)
    .padding(10);

    content = content.push(container(add_macro_row).style(container::rounded_box));

    content.into()
}

fn view_resolved_actions(profile: &GameProfile) -> Element<'_, Message> {
    let actions = profile.resolve_actions();
    if actions.is_empty() {
        return text("No actions resolved. Are your bindings loaded?")
            .style(text::danger)
            .into();
    }

    let mut list = Column::new().spacing(5);
    let mut keys: Vec<String> = actions.keys().cloned().collect();
    keys.sort();

    for name in keys {
        if let Some(binding) = actions.get(&name) {
            let key_str = binding.primary_key.as_deref().unwrap_or("None");
            let mods = if binding.modifiers.is_empty() {
                "".to_string()
            } else {
                format!(" (+ {:?})", binding.modifiers)
            };

            list = list.push(row![
                text(name.to_string()).width(Length::Fixed(150.0)),
                text("👉").width(Length::Fixed(30.0)),
                text(format!("{}{}", key_str, mods)).style(text::success),
            ]);
        }
    }

    container(list).padding(10).into()
}
