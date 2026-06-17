use std::fs;
use crate::session::{load_session, session_path};

pub async fn execute_history() -> Result<(), Box<dyn std::error::Error>> {
    let session = load_session();
    if session.messages.is_empty() {
        println!("No history for this session.\n");
    } else {
        println!();
        for msg in &session.messages {
            let role = msg["role"].as_str().unwrap_or("unknown");
            if role == "system" || role == "tool" {
                continue;
            }

            let label = match role {
                "user" => "You",
                "assistant" => "AI",
                _ => role,
            };
            let color = if role == "user" {
                "\x1b[33m"
            } else {
                "\x1b[36m"
            };

            let content = msg["content"].as_str().unwrap_or("");
            if !content.is_empty() {
                println!("{}[{}]\x1b[0m {}\n", color, label, content);
            }
        }
    }
    println!(
        "\x1b[2mSession file: {}\x1b[0m \n",
        session_path().display()
    );
    Ok(())
}

pub async fn execute_clear() -> Result<(), Box<dyn std::error::Error>> {
    let p = session_path();
    if p.exists() {
        fs::remove_file(&p).ok();
        println!("✓ Session history cleared.");
    } else {
        println!("No history to clear.\n");
    }
    Ok(())
}

pub async fn execute_clean_all() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir();
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(temp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("ai_session_") && name.ends_with(".json") {
                    if fs::remove_file(path).is_ok() {
                        count += 1;
                    }
                }
            }
        }
    }
    println!("✓ Cleared {} session(s) across all terminals.\n", count);
    Ok(())
}
