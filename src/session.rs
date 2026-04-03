use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Serialize, Deserialize, Default)]
pub struct Session {
    pub messages: Vec<Value>,
}

pub fn parent_pid() -> u32 {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, false);
    let my_pid = Pid::from_u32(std::process::id());
    sys.process(my_pid)
        .and_then(|p| p.parent())
        .map(|pid| pid.as_u32())
        .unwrap_or_else(std::process::id)
}

pub fn session_path() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("ai_session_{}.json", parent_pid()));
    p
}

pub fn load_session() -> Session {
    let p = session_path();
    if p.exists() {
        let s = fs::read_to_string(p).unwrap_or_default();
        serde_json::from_str(&s).unwrap_or_default()
    } else {
        Session::default()
    }
}

pub fn save_session(session: &Session) {
    let s = serde_json::to_string_pretty(session).unwrap();
    fs::write(session_path(), s).expect("Failed to save session");
}

pub fn clean_orphaned_sessions() {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, false);

    let temp_dir = std::env::temp_dir();
    if let Ok(entries) = fs::read_dir(temp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("ai_session_") && name.ends_with(".json") {
                    let pid_str = &name["ai_session_".len()..name.len() - ".json".len()];
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        // If the process is dead, delete the orphaned session file
                        if sys.process(Pid::from_u32(pid)).is_none() {
                            let _ = fs::remove_file(path);
                        }
                    }
                }
            }
        }
    }
}
