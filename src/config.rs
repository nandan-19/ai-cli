use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub api_key: Option<String>,
    pub model: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: None,
            model: "openai/gpt-oss-20b".to_string(),
        }
    }
}

pub fn config_path() -> PathBuf {
    let mut p = dirs::home_dir().expect("Cannot resolve home directory");
    p.push(".terminal_ai.json");
    p
}

pub fn load_config() -> Config {
    let p = config_path();
    if p.exists() {
        let s = fs::read_to_string(p).unwrap_or_default();
        serde_json::from_str(&s).unwrap_or_default()
    } else {
        Config::default()
    }
}

pub fn save_config(cfg: &Config) {
    let s = serde_json::to_string_pretty(cfg).unwrap();
    fs::write(config_path(), s).expect("Failed to save config");
}
