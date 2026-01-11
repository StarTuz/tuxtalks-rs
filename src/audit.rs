use anyhow::Result;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

/// Write an entry to the audit log
pub fn log(entry: &str) -> Result<()> {
    // Determine config directory (respecting XDG)
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".config"));
    let log_dir = config_dir.join("tuxtalks");
    std::fs::create_dir_all(&log_dir)?;

    let log_path = log_dir.join("audit.log");

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;

    writeln!(
        file,
        "[{}] {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        entry
    )?;
    Ok(())
}

/// Log an action taken by the AI agent with an associated risk tier and justification
pub fn log_agent_action(tier: u8, action: &str, justification: &str) -> Result<()> {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".config"));
    let log_dir = config_dir.join("tuxtalks");
    std::fs::create_dir_all(&log_dir)?;

    let log_path = log_dir.join("agent_actions.log");

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;

    writeln!(
        file,
        "[{}] [TIER {}] ACTION: {} | JUSTIFICATION: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        tier,
        action,
        justification
    )?;
    Ok(())
}
