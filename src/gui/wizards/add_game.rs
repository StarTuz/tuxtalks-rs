use crate::gui::Message;
use iced::widget::{
    button, column, container, pick_list, row, scrollable, text, text_input, Column, Space,
};
use iced::{Alignment, Element, Length};
use sysinfo::{ProcessesToUpdate, System};

#[derive(Debug, Clone)]
pub struct AddGameWizard {
    pub scanned_processes: Vec<ScannedProcess>,
    pub selected_pid: Option<u32>,

    // Form Data
    pub game_type_input: String,
    pub game_name_input: String,
    pub process_name_input: String,
    pub profile_name_input: String,
    pub runtime_input: String,
    pub bindings_path_input: String,
}

#[derive(Debug, Clone)]
pub struct ScannedProcess {
    pub pid: u32,
    pub name: String,
    pub cmd: String,
}

#[derive(Debug, Clone)]
pub enum WizardMessage {
    ScanProcesses,
    SelectProcess(u32),
    Cancel,
    Finish,

    // Form inputs
    GameTypeChanged(String),
    GameNameChanged(String),
    ProcessNameChanged(String),
    ProfileNameChanged(String),
    RuntimeChanged(String),
    BindingsPathChanged(String),
}

impl Default for AddGameWizard {
    fn default() -> Self {
        Self::new()
    }
}

impl AddGameWizard {
    pub fn new() -> Self {
        Self {
            scanned_processes: Vec::new(),
            selected_pid: None,
            game_type_input: "Generic / Other".to_string(),
            game_name_input: String::new(),
            process_name_input: String::new(),
            profile_name_input: String::new(),
            runtime_input: "Native Linux".to_string(),
            bindings_path_input: String::new(),
        }
    }

    pub fn update(&mut self, message: WizardMessage) {
        match message {
            WizardMessage::ScanProcesses => {
                let mut system = System::new();
                system.refresh_processes(ProcessesToUpdate::All, true);

                self.scanned_processes = system
                    .processes()
                    .iter()
                    .map(|(pid, proc)| ScannedProcess {
                        pid: pid.as_u32(),
                        name: proc.name().to_string_lossy().to_string(),
                        cmd: proc
                            .cmd()
                            .iter()
                            .map(|s| s.to_string_lossy())
                            .collect::<Vec<_>>()
                            .join(" "),
                    })
                    .collect();

                // Sort by name
                self.scanned_processes.sort_by(|a, b| a.name.cmp(&b.name));
            }
            WizardMessage::SelectProcess(pid) => {
                self.selected_pid = Some(pid);
                if let Some(proc) = self.scanned_processes.iter().find(|p| p.pid == pid) {
                    self.process_name_input = proc.name.clone();
                    // Heuristic for game name?
                    self.game_name_input = proc.name.clone();
                    // Default profile name
                    if self.profile_name_input.is_empty() {
                        self.profile_name_input = "Default".to_string();
                    }
                }
            }
            WizardMessage::GameTypeChanged(val) => self.game_type_input = val,
            WizardMessage::GameNameChanged(val) => self.game_name_input = val,
            WizardMessage::ProcessNameChanged(val) => self.process_name_input = val,
            WizardMessage::ProfileNameChanged(val) => self.profile_name_input = val,
            WizardMessage::RuntimeChanged(val) => self.runtime_input = val,
            WizardMessage::BindingsPathChanged(val) => self.bindings_path_input = val,
            _ => {}
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content = column![
            // Header
            text("Add Game Profile").size(24),
            // Step 1: Process List
            container(self.view_process_list())
                .height(Length::FillPortion(1))
                .style(container::rounded_box)
                .padding(10),
            Space::with_height(10),
            // Step 2: Configuration
            container(self.view_form())
                .height(Length::FillPortion(1))
                .style(container::rounded_box)
                .padding(10),
            // Footer Actions
            row![
                button("Cancel")
                    .style(button::danger)
                    .on_press(Message::Wizard(WizardMessage::Cancel)),
                Space::with_width(Length::Fill),
                button("💾 Add Game Profile")
                    .style(button::success)
                    .on_press(Message::Wizard(WizardMessage::Finish))
            ]
        ]
        .spacing(15);

        container(content)
            .width(Length::Fixed(900.0))
            .height(Length::Fixed(700.0)) // Taller to accommodate both views
            .padding(20)
            .style(container::rounded_box)
            .into()
    }

    fn view_process_list(&self) -> Element<'_, Message> {
        let mut list_col = Column::new().spacing(5);

        // Header Row
        list_col = list_col.push(
            row![
                text("PID")
                    .width(Length::Fixed(60.0))
                    .style(text::secondary),
                text("Process Name")
                    .width(Length::Fixed(200.0))
                    .style(text::secondary),
                text("Command Line / Path")
                    .width(Length::Fill)
                    .style(text::secondary),
            ]
            .spacing(10),
        );

        // List Items
        for proc in &self.scanned_processes {
            let is_selected = Some(proc.pid) == self.selected_pid;
            let style = if is_selected {
                button::primary
            } else {
                button::secondary
            };

            list_col = list_col.push(
                button(
                    row![
                        text(proc.pid.to_string()).width(Length::Fixed(60.0)),
                        text(&proc.name).width(Length::Fixed(200.0)),
                        text(&proc.cmd).width(Length::Fill),
                    ]
                    .spacing(10),
                )
                .style(style)
                .on_press(Message::Wizard(WizardMessage::SelectProcess(proc.pid)))
                .padding(5)
                .width(Length::Fill),
            );
        }

        column![
            row![
                text("Step 1: Select Running Game Process").size(18),
                Space::with_width(Length::Fill),
                button("🔄 Scan Processes").on_press(Message::Wizard(WizardMessage::ScanProcesses))
            ]
            .align_y(Alignment::Center),
            Space::with_height(5),
            scrollable(list_col)
        ]
        .spacing(10)
        .into()
    }

    fn view_form(&self) -> Element<'_, Message> {
        let label_width = Length::Fixed(160.0);

        column![
            text("Step 2: Configuration").size(18),
            Space::with_height(10),
            // Game Type
            row![
                text("Game Type:").width(label_width),
                pick_list(
                    vec![
                        "X4 Foundations (Steam Proton)".to_string(),
                        "X4 Foundations (Steam Native)".to_string(),
                        "Elite Dangerous (Steam)".to_string(),
                        "Generic / Other".to_string()
                    ],
                    Some(self.game_type_input.clone()),
                    |v| Message::Wizard(WizardMessage::GameTypeChanged(v))
                )
                .width(Length::Fill)
            ]
            .align_y(Alignment::Center)
            .spacing(10),
            // Game Name
            row![
                text("Game Name:").width(label_width),
                text_input("", &self.game_name_input)
                    .on_input(|v| Message::Wizard(WizardMessage::GameNameChanged(v)))
            ]
            .align_y(Alignment::Center)
            .spacing(10),
            // Process Name
            row![
                text("Process Name:").width(label_width),
                text_input("", &self.process_name_input)
                    .on_input(|v| Message::Wizard(WizardMessage::ProcessNameChanged(v)))
            ]
            .align_y(Alignment::Center)
            .spacing(10),
            // Profile Name
            row![
                text("Binding Profile Name:").width(label_width),
                column![
                    text_input("", &self.profile_name_input)
                        .on_input(|v| Message::Wizard(WizardMessage::ProfileNameChanged(v))),
                    text("(defaults to binding file name, e.g. 'inputmap.xml')")
                        .size(12)
                        .style(text::secondary)
                ]
                .width(Length::Fill)
            ]
            .align_y(Alignment::Start)
            .spacing(10),
            // Runtime Environment
            row![
                text("Runtime Environment:").width(label_width),
                pick_list(
                    vec!["Native Linux".to_string(), "Proton/Wine".to_string()],
                    Some(self.runtime_input.clone()),
                    |v| Message::Wizard(WizardMessage::RuntimeChanged(v))
                )
                .width(Length::Fill)
            ]
            .align_y(Alignment::Center)
            .spacing(10),
            // Bindings Path
            row![
                text("Bindings Path:").width(label_width),
                text_input("", &self.bindings_path_input)
                    .on_input(|v| Message::Wizard(WizardMessage::BindingsPathChanged(v))),
                button("Browse"),  // Placeholder
                button("🔍 Scan")  // Placeholder
            ]
            .align_y(Alignment::Center)
            .spacing(10),
        ]
        .spacing(10)
        .into()
    }
}
