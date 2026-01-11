use tokio::time::Instant;
use tuxtalks::commands::{CommandProcessor, ProcessResult};

#[tokio::test]
async fn test_asr_flood_fuzz() {
    let mut processor = CommandProcessor::new().expect("Failed to create processor");

    // Simulate a flood of random garbage text
    let garbage = [
        "asdfghjkl",
        "!!! @@@ ###",
        "1234567890",
        "extremely long string that doesn't mean anything to the system at all but might cause buffer issues if we were in C but we are in Rust so it's just a long string",
        "",
        " ",
    ];

    for text in garbage {
        let res = processor.process(text).await;
        assert!(matches!(res, ProcessResult::NotFound));
    }

    // Simulate high-frequency "safe" commands to check rate limiting
    // Note: Rate limiting is currently implemented in mod.rs (GUI),
    // but the processor should stay stable.
    let commands = ["play artist radiohead", "next", "previous", "stop"];

    let start = Instant::now();
    for i in 0..100 {
        let cmd = commands[i % commands.len()];
        let _ = processor.process(cmd).await;
    }
    let elapsed = start.elapsed();
    println!("Processed 100 commands in {:?}", elapsed);

    // Stability check: processor should still be functional
    let res = processor.process("stop").await;
    match res {
        ProcessResult::Success(name) => assert_eq!(name, "stop"),
        _ => panic!("Processor died after flood test"),
    }
}

#[tokio::test]
async fn test_high_risk_confirmation_fuzz() {
    let mut processor = CommandProcessor::new().expect("Failed to create processor");

    // Add a dangerous command for testing
    // In a real scenario these come from profiles
    processor.add_demo_bindings(); // Adds boost/fire, not dangerous.

    // Let's add "self destruct" manually
    use tuxtalks::commands::Command;
    processor.add_command(Command::Action {
        name: "self destruct".to_string(),
        triggers: vec!["self destruct".to_string()],
        key: "X".to_string(),
        modifiers: vec![],
    });

    let res = processor.process("self destruct").await;
    match res {
        ProcessResult::ConfirmationRequired { action, .. } => {
            assert_eq!(action, "self destruct");
        }
        _ => panic!(
            "Dangerous command did not trigger confirmation. Found: {:?}",
            res
        ),
    }
}
#[tokio::test]
async fn test_entity_verification_fuzz() {
    let mut processor = CommandProcessor::new().expect("Failed to create processor");

    // Test that media commands fail if the entity doesn't exist (Wendy Chisholm requirement)
    // "play artist NON_EXISTENT"
    let res = processor
        .process("play artist non_existent_artist_999")
        .await;
    assert!(matches!(res, ProcessResult::NotFound));

    // "play album NON_EXISTENT"
    let res = processor.process("play album mystery_album_123").await;
    assert!(matches!(res, ProcessResult::NotFound));
}

#[tokio::test]
async fn test_phonetic_matching_fuzz() {
    let mut processor = CommandProcessor::new().expect("Failed to create processor");

    // Add a command for testing
    use tuxtalks::commands::Command;
    processor.add_command(Command::Action {
        name: "fire phasers".to_string(),
        triggers: vec!["fire phasers".to_string()],
        key: "F".to_string(),
        modifiers: vec![],
    });

    // Test exact match
    let res = processor.process("fire phasers").await;
    match res {
        ProcessResult::Success(name) => assert_eq!(name, "fire phasers"),
        _ => panic!("Exact match failed"),
    }

    // Test phonetic match (ASR error: "fire physics")
    // Wendy Chisholm requirement: Advanced Phonetic Matching
    let res = processor.process("fire physics").await;
    match res {
        ProcessResult::Success(name) => assert_eq!(name, "fire phasers"),
        _ => panic!(
            "Phonetic match failed for 'fire physics'. Result: {:?}",
            res
        ),
    }

    // Test phonetic match (ASR error: "fyer phasers")
    let res = processor.process("fyer phasers").await;
    match res {
        ProcessResult::Success(name) => assert_eq!(name, "fire phasers"),
        _ => panic!(
            "Phonetic match failed for 'fyer phasers'. Result: {:?}",
            res
        ),
    }
}
