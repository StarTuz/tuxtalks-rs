use crate::gui::{Message, TuxTalksApp};
use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{Alignment, Element, Length};

pub fn view(app: &TuxTalksApp) -> Element<'_, Message> {
    let mut content = Column::new().spacing(20);
    content = content.push(text("Content Packs (LAL Manager)").size(28));
    content = content.push(
        text("Manage Language, Audio, and Logic packs.")
            .size(16)
            .style(text::secondary),
    );

    // Mocked list of packs
    let mut list = Column::new().spacing(10);

    // Installed Packs
    list = list.push(text("Installed Packs").size(20));

    let installed_packs = app.lal_manager.list_packs();

    if installed_packs.is_empty() {
        list = list.push(
            text("No content packs installed.")
                .style(text::secondary)
                .size(14),
        );
    } else {
        for pack in installed_packs {
            list = list.push(
                container(
                    row![
                        column![
                            text(pack.name.clone()).size(18),
                            text(pack.author.clone()).size(14).style(text::secondary),
                        ]
                        .width(Length::Fill),
                        text(pack.version.clone()).style(text::secondary),
                        button("Remove")
                            .style(button::danger)
                            .on_press(Message::RemovePack(pack.name.clone())),
                    ]
                    .align_y(Alignment::Center)
                    .spacing(20),
                )
                .padding(15)
                .style(container::rounded_box),
            );
        }
    }

    list = list.push(text("Available Online").size(20));

    let available = vec![
        (
            "Star Trek: TNG Voice",
            "v1.0",
            "Computer voice feedback sounds",
        ),
        ("X4 Foundations: Pro", "v1.2", "Advanced trading macros"),
    ];

    for (name, ver, desc) in available {
        list = list.push(
            container(
                row![
                    column![
                        text(name).size(18),
                        text(desc).size(14).style(text::secondary),
                    ]
                    .width(Length::Fill),
                    text(ver).style(text::secondary),
                    button("Install").on_press(Message::InstallPack(name.to_string())),
                ]
                .align_y(Alignment::Center)
                .spacing(20),
            )
            .padding(15)
            .style(container::rounded_box),
        );
    }

    content = content.push(scrollable(list).height(Length::Fill));

    content.into()
}
