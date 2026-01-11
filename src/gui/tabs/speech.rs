use crate::gui::{Message, TuxTalksApp};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length};

pub fn view(app: &TuxTalksApp) -> Element<'_, Message> {
    let mut content = column![].spacing(20);
    content = content.push(text("Speech Engines").size(24));

    // Split View for ASR and TTS
    let asr_section = build_engine_section(
        "Speech Recognition (ASR)",
        vec![
            "vosk".to_string(),
            "wyoming".to_string(),
            "speechd_ng".to_string(),
        ],
        &app.speech_state.selected_asr,
        true,
    );

    let tts_section = build_engine_section(
        "Text-to-Speech (TTS)",
        vec![
            "piper".to_string(),
            "speechd_ng".to_string(),
            "system".to_string(),
        ],
        &app.speech_state.selected_tts,
        false,
    );

    let engines_row = row![asr_section, tts_section]
        .spacing(20)
        .height(Length::FillPortion(2));

    content = content.push(engines_row);

    // Wyoming Settings
    let wyoming_settings = container(
        column![
            text("Wyoming Settings (External Server)").size(20),
            Space::with_height(10),
            row![
                text("Host:").width(Length::Fixed(80.0)),
                text_input("localhost", &app.speech_state.wyoming_host_input)
                    .width(Length::Fixed(200.0))
                    .on_input(Message::WyomingHostChanged),
                text("Port:").width(Length::Fixed(80.0)),
                text_input("10300", &app.speech_state.wyoming_port_input)
                    .width(Length::Fixed(100.0))
                    .on_input(Message::WyomingPortChanged),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(10),
    )
    .padding(15)
    .style(container::rounded_box)
    .width(Length::Fill);

    content = content.push(wyoming_settings);

    // Apply Button
    content = content.push(row![
        Space::with_width(Length::Fill),
        button("Apply Speech Settings")
            .on_press(Message::ApplySpeechSettings)
            .padding(10)
    ]);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn build_engine_section<'a>(
    title: &'a str,
    engines: Vec<String>,
    selected: &'a str,
    is_asr: bool,
) -> Element<'a, Message> {
    // 1. List of engines
    let list = column(
        engines
            .iter()
            .map(|engine| {
                let is_selected = engine == selected;
                button(text(engine.clone()))
                    .style(if is_selected {
                        button::primary
                    } else {
                        button::secondary
                    })
                    .width(Length::Fill)
                    .on_press(if is_asr {
                        Message::SelectAsrEngine(engine.clone())
                    } else {
                        Message::SelectTtsEngine(engine.clone())
                    })
                    .into()
            })
            .collect::<Vec<_>>(),
    )
    .spacing(5);

    let selection_box = container(scrollable(list))
        .height(Length::Fixed(150.0))
        .style(container::rounded_box)
        .padding(5);

    // 2. Metadata Card
    let (name, desc, pros, cons) = get_engine_metadata(selected);

    let metadata_card = container(
        column![
            text(format!("Name: {}", name)).size(16),
            text(desc).size(14).style(text::secondary),
            Space::with_height(10),
            text("✅ Pros:").size(14).style(text::success),
            text(pros).size(14),
            Space::with_height(5),
            text("❌ Cons:").size(14).style(text::danger),
            text(cons).size(14),
        ]
        .spacing(5),
    )
    .height(Length::Fill)
    .padding(10);

    container(column![text(title).size(16), selection_box, metadata_card].spacing(10))
        .width(Length::FillPortion(1))
        .padding(10)
        .style(container::rounded_box)
        .into()
}

fn get_engine_metadata(engine: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    match engine {
        "vosk" => (
            "Vosk (Offline)",
            "Efficient offline speech recognition. Good accuracy for commands.",
            "• Completely Offline\n• Low Latency\n• No GPU required",
            "• Less accurate than Whisper\n• Fixed vocabulary models",
        ),
        "wyoming" => (
            "Wyoming (External)",
            "Connects to a Wyoming protocol server (e.g. Whisper, Piper).",
            "• High quality AI models\n• Flexible backend support",
            "• Requires external server\n• Network dependency",
        ),
        "speechd_ng" => (
            "SpeechD-NG (Unified)",
            "Uses speechd-ng daemon for ASR/TTS. Best paired with speechd-ng TTS.",
            "• Unified with TTS\n• Low resource usage\n• D-Bus Integration",
            "• Requires speechd-ng daemon running",
        ),
        "piper" => (
            "Piper (Neural TTS)",
            "Fast, local neural text-to-speech system. High quality natural voices.",
            "• High quality voices\n• Fast synthesis\n• Offline",
            "• Limited voice selection compared to cloud",
        ),
        "system" => (
            "System TTS",
            "Uses the system standard speech dispatcher.",
            "• Integrated with OS\n• Zero config",
            "• Robotic voices usually",
        ),
        _ => ("Unknown", "No description available", "", ""),
    }
}
