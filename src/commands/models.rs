use crate::config::{Config, save_config};
use reqwest::Client;
use serde_json::Value;
use std::io::Write;

pub async fn execute_set_key(key: String, config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
    config.api_key = Some(key);
    save_config(config);
    println!("✓ API key saved.\n");
    Ok(())
}

pub async fn execute_set_model(model: Option<String>, config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(m) = model {
        config.model = m.clone();
        save_config(config);
        println!("✓ Model switched to {}.\n", m);
    } else {
        let api_key = match &config.api_key {
            Some(k) => k.clone(),
            None => {
                println!("No API key set. Run:  ai set-key YOUR_KEY");
                return Ok(());
            }
        };

        println!("\x1b[2mFetching available models from Groq...\x1b[0m");

        let client = Client::new();
        let res = match client
            .get("https://api.groq.com/openai/v1/models")
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("\x1b[31mFailed to fetch models:\x1b[0m {}", e);
                println!(
                    "Fallback: continuing with default model ({}).",
                    config.model
                );
                return Ok(());
            }
        };

        if !res.status().is_success() {
            let err_text = res.text().await.unwrap_or_default();
            eprintln!("\x1b[31mAPI Error:\x1b[0m {}", err_text);
            println!(
                "Fallback: continuing with default model ({}).",
                config.model
            );
            return Ok(());
        }

        let data: Value = match res.json().await {
            Ok(d) => d,
            Err(e) => {
                eprintln!("\x1b[31mFailed to parse response:\x1b[0m {}", e);
                println!(
                    "Fallback: continuing with default model ({}).",
                    config.model
                );
                return Ok(());
            }
        };

        let mut models = Vec::new();
        if let Some(arr) = data["data"].as_array() {
            for m in arr {
                if let Some(id) = m["id"].as_str() {
                    // Filter out audio models since this is a text CLI
                    if id.contains("whisper") {
                        continue;
                    }
                    let owner = m["owned_by"].as_str().unwrap_or("unknown");
                    let ctx = m["context_window"].as_u64().unwrap_or(0);
                    models.push((id.to_string(), owner.to_string(), ctx));
                }
            }
        }

        if models.is_empty() {
            println!("No models found via API. Returning to default.");
            return Ok(());
        }

        // Sort models alphabetically
        models.sort_by(|a, b| a.0.cmp(&b.0));

        let mut max_id_len = 8; // "Model ID" length
        let mut max_owner_len = 5; // "Owner" length
        for (id, owner, _) in &models {
            if id.len() > max_id_len {
                max_id_len = id.len();
            }
            if owner.len() > max_owner_len {
                max_owner_len = owner.len();
            }
        }

        println!("\n\x1b[1mAvailable Models\x1b[0m");
        println!(
            "{:<4} | {:<id_w$} | {:<ow_w$} | {:<14}",
            "No.",
            "Model ID",
            "Owner",
            "Context Window",
            id_w = max_id_len,
            ow_w = max_owner_len
        );
        println!(
            "{:-<4}-+-{:-<id_w$}-+-{:-<ow_w$}-+-{:-<14}",
            "",
            "",
            "",
            "",
            id_w = max_id_len,
            ow_w = max_owner_len
        );

        for (i, (id, owner, ctx)) in models.iter().enumerate() {
            let ctx_str = if *ctx > 0 {
                format!("{}", ctx)
            } else {
                "Unknown".to_string()
            };
            println!(
                "{:<4} | \x1b[36m{:<id_w$}\x1b[0m | {:<ow_w$} | {:<14}",
                i + 1,
                id,
                owner,
                ctx_str,
                id_w = max_id_len,
                ow_w = max_owner_len
            );
        }
        println!();

        print!("Enter the number of the model to use (or press enter to cancel): ");
        let _ = std::io::stdout().flush();
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            let input = input.trim();
            if input.is_empty() {
                println!("Cancelled. Keeping model: {}\n", config.model);
                return Ok(());
            }
            if let Ok(idx) = input.parse::<usize>() {
                if idx > 0 && idx <= models.len() {
                    let selected = &models[idx - 1].0;
                    config.model = selected.clone();
                    save_config(config);
                    println!("✓ Model switched to {}.\n", selected);
                } else {
                    println!(
                        "\x1b[31mInvalid selection.\x1b[0m Keeping model: {}\n",
                        config.model
                    );
                }
            } else {
                println!(
                    "\x1b[31mInvalid input.\x1b[0m Keeping model: {}\n",
                    config.model
                );
            }
        } else {
            println!("Cancelled. Keeping model: {}\n", config.model);
        }
    }
    Ok(())
}

pub async fn execute_stream_toggle(config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
    config.streaming = !config.streaming;
    save_config(config);
    let mode = if config.streaming {
        "\x1b[32mstreaming\x1b[0m (live output)"
    } else {
        "\x1b[35mnon-streaming\x1b[0m (rendered Markdown)"
    };
    println!("✓ Switched to {} mode.", mode);
    Ok(())
}
