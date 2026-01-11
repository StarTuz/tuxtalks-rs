use crate::gui::{Message, TuxTalksApp};
use iced::widget::{button, column, container, row, text, Radio};
use iced::{Alignment, Element, Length};

#[derive(Debug, Clone)]
pub struct SetupWizard {
    pub step: WizardStep,
    pub language: Language,
    pub asr_engine: String,
    pub player_backend: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WizardStep {
    Welcome,
    Language,
    Asr,
    Player,
    Finish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    EnglishUS,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "English (US)")
    }
}

#[derive(Debug, Clone)]
pub enum WizardMessage {
    NextStep,
    LanguageSelected(Language),
    AsrSelected(String), // String is not Copy, so can't use Radio directly? Use PickList or custom buttons
    PlayerSelected(String), // Same
    Finish,
}

impl Default for SetupWizard {
    fn default() -> Self {
        Self::new()
    }
}

impl SetupWizard {
    pub fn new() -> Self {
        Self {
            step: WizardStep::Welcome,
            language: Language::EnglishUS,
            asr_engine: "vosk".to_string(),
            player_backend: "strawberry".to_string(),
        }
    }

    pub fn view<'a>(&self, _app: &TuxTalksApp) -> Element<'a, Message> {
        let content = match self.step {
            WizardStep::Welcome => column![
                text("Welcome to TuxTalks!").size(32),
                text("Let's get you set up with voice control for your games.").size(18),
            ]
            .spacing(20),
            WizardStep::Language => column![
                text("Choose Your Language").size(24),
                Radio::new(
                    "English (US)",
                    Language::EnglishUS,
                    Some(self.language),
                    |l| Message::SetupWizard(WizardMessage::LanguageSelected(l))
                ),
            ]
            .spacing(20),
            WizardStep::Asr => column![
                text("Select Speech Engine").size(24),
                // Mock buttons for now as Radio requires Copy
                button("Vosk (Offline)").on_press(Message::SetupWizard(
                    WizardMessage::AsrSelected("vosk".to_string())
                )),
                button("Wyoming (Docker)").on_press(Message::SetupWizard(
                    WizardMessage::AsrSelected("wyoming".to_string())
                )),
            ]
            .spacing(20),
            WizardStep::Player => column![
                text("Select Media Player").size(24),
                button("Strawberry").on_press(Message::SetupWizard(WizardMessage::PlayerSelected(
                    "strawberry".to_string()
                ))),
                button("JRiver Media Center").on_press(Message::SetupWizard(
                    WizardMessage::PlayerSelected("jriver".to_string())
                )),
            ]
            .spacing(20),
            WizardStep::Finish => column![
                text("All Set!").size(32),
                text("Click Finish to update your settings and start.").size(18),
            ]
            .spacing(20),
        };

        let action_button = if self.step == WizardStep::Finish {
            button("Finish").on_press(Message::SetupWizard(WizardMessage::Finish))
        } else {
            button("Next").on_press(Message::SetupWizard(WizardMessage::NextStep))
        };

        container(
            column![content, row![action_button].spacing(20)]
                .align_x(Alignment::Center)
                .spacing(30),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    }
}
