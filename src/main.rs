mod cli;
mod config;
mod session;
mod tools;

use clap::Parser;
use futures::stream::StreamExt;
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command as TokioCommand;

use cli::{Cli, Commands};
use config::{load_config, save_config};
use session::{clean_orphaned_sessions, load_session, save_session, session_path};
use tools::{execute_tool, get_tools, ToolCallTracker};

const SYSTEM_PROMPT: &str = "You are a helpful CLI AI assistant. Provide clean, \
    readable, terminal-friendly output. Keep answers concise by default, but \
    provide detailed explanations if the user explicitly asks. Avoid overly \
    complex markdown tables. You have access to tools that let you act as an agent \
    on the user's local machine. Use these tools when requested or when they help \
    you fulfill the objective.";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Run automatic cleanup in the background occasionally or simply synchronously
    // because it's fast.
    clean_orphaned_sessions();

    let cli = Cli::parse();
    let mut config = load_config();

    // ── Subcommands ─────────────────────────────────────────────────────────
    if let Some(cmd) = cli.command {
        match cmd {
            Commands::SetKey { key } => {
                config.api_key = Some(key);
                save_config(&config);
                println!("✓ API key saved.");
            }
            Commands::SetModel { model } => {
                config.model = model.clone();
                save_config(&config);
                println!("✓ Model switched to {}.", model);
            }
            Commands::History => {
                let session = load_session();
                if session.messages.is_empty() {
                    println!("No history for this session.");
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
                        let color = if role == "user" { "\x1b[33m" } else { "\x1b[36m" };

                        let content = msg["content"].as_str().unwrap_or("");
                        if !content.is_empty() {
                            println!("{}[{}]\x1b[0m {}\n", color, label, content);
                        }
                    }
                }
                println!("\x1b[2mSession file: {}\x1b[0m", session_path().display());
            }
            Commands::Clear => {
                let p = session_path();
                if p.exists() {
                    fs::remove_file(&p).ok();
                    println!("✓ Session history cleared.");
                } else {
                    println!("No history to clear.");
                }
            }
            Commands::CleanAll => {
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
                println!("✓ Cleared {} session(s) across all terminals.", count);
            }
            Commands::Rec { cmd_args } => {
                if cmd_args.is_empty() {
                    println!("Please provide a command to run, e.g., ai rec cargo build");
                    return Ok(());
                }

                let full_cmd = cmd_args.join(" ");
                println!("\x1b[2m[ai rec] Running: {}\x1b[0m\n", full_cmd);

                let is_windows = cfg!(target_os = "windows");
                let (exec_cmd, exec_args) = if is_windows {
                    ("cmd", vec!["/C", &full_cmd])
                } else {
                    ("sh", vec!["-c", &full_cmd])
                };

                let child = TokioCommand::new(exec_cmd)
                    .args(&exec_args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn();

                match child {
                    Ok(mut child) => {
                        let mut stdout = child.stdout.take().unwrap();
                        let mut stderr = child.stderr.take().unwrap();

                        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

                        let tx_out = tx.clone();
                        tokio::spawn(async move {
                            let mut buf = [0; 1024];
                            while let Ok(n) = stdout.read(&mut buf).await {
                                if n == 0 {
                                    break;
                                }
                                let _ = tokio::io::stdout().write_all(&buf[..n]).await;
                                let _ =
                                    tx_out.send(String::from_utf8_lossy(&buf[..n]).into_owned());
                            }
                        });

                        let tx_err = tx.clone();
                        tokio::spawn(async move {
                            let mut buf = [0; 1024];
                            while let Ok(n) = stderr.read(&mut buf).await {
                                if n == 0 {
                                    break;
                                }
                                let _ = tokio::io::stderr().write_all(&buf[..n]).await;
                                let _ =
                                    tx_err.send(String::from_utf8_lossy(&buf[..n]).into_owned());
                            }
                        });

                        drop(tx);

                        let mut full_output = String::new();
                        while let Some(chunk) = rx.recv().await {
                            full_output.push_str(&chunk);
                        }

                        let status = child.wait().await?;

                        let context = format!(
                            "User ran command: `{}`\nExit status: {}\nOutput:\n```\n{}\n```",
                            full_cmd, status, full_output
                        );

                        let mut session = load_session();
                        session.messages.push(json!({
                            "role": "user",
                            "content": context,
                        }));
                        save_session(&session);
                        println!("\n\x1b[2m[ai rec] Saved output to session context.\x1b[0m");
                    }
                    Err(e) => {
                        let err_msg = format!("Failed to run command '{}': {}", full_cmd, e);
                        eprintln!("\n\x1b[31m{}\x1b[0m", err_msg);

                        let context = format!(
                            "User attempted to run command: `{}`\nBut it failed to spawn:\n{}",
                            full_cmd, err_msg
                        );

                        let mut session = load_session();
                        session.messages.push(json!({
                            "role": "user",
                            "content": context,
                        }));
                        save_session(&session);
                        println!("\n\x1b[2m[ai rec] Saved error to session context.\x1b[0m");
                    }
                }
            }
        }
        return Ok(());
    }

    if cli.prompt.is_empty() {
        println!("Usage:");
        println!("  ai your question (no quotes needed!)");
        println!("  ai -p follow-up            include session history as context");
        println!("  ai rec <command>           run command and save output to history");
        println!("  ai set-key YOUR_KEY");
        println!("  ai set-model MODEL_NAME");
        println!("  ai history");
        println!("  ai clear");
        println!("  ai clean-all               clear all saved session histories across all terminals");
        return Ok(());
    }

    let api_key = match &config.api_key {
        Some(k) => k.clone(),
        None => {
            println!("No API key set. Run:  ai set-key YOUR_KEY");
            return Ok(());
        }
    };

    let prompt = cli.prompt.join(" ");

    let mut session = load_session();
    let mut messages: Vec<Value> = vec![json!({
        "role": "system",
        "content": SYSTEM_PROMPT
    })];

    if cli.prev_context {
        for msg in &session.messages {
            messages.push(msg.clone());
        }
    }

    let user_msg = json!({ "role": "user", "content": prompt });
    messages.push(user_msg.clone());
    
    // Always append the new user query to the running session
    session.messages.push(user_msg);
    save_session(&session);
    
    println!("\n\x1b[36m[{}]\x1b[0m\n", config.model);

    // ── Agent Loop ───────────────────────────────────────────────────────────

    let client = Client::new();

    'chat: loop {
        let payload = json!({
            "model": config.model,
            "messages": messages,
            "stream": true,
            "tools": get_tools(),
            "tool_choice": "auto"
        });

        let request = client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&payload);

        let mut es = match EventSource::new(request) {
            Ok(src) => src,
            Err(e) => {
                eprintln!("Event stream error: {}", e);
                break;
            }
        };

        let mut full_response = String::new();
        let mut tool_calls: HashMap<usize, ToolCallTracker> = HashMap::new();

        while let Some(event) = es.next().await {
            match event {
                Ok(Event::Open) => {}
                Ok(Event::Message(msg)) => {
                    if msg.data == "[DONE]" {
                        es.close();
                        break;
                    }
                    if let Ok(data) = serde_json::from_str::<Value>(&msg.data) {
                        if let Some(choices) = data.get("choices") {
                            if let Some(delta) = choices[0].get("delta") {
                                // Accumulate string content
                                if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
                                    print!("{}", text);
                                    io::stdout().flush().unwrap();
                                    full_response.push_str(text);
                                }

                                // Accumulate tool calls
                                if let Some(tcs) =
                                    delta.get("tool_calls").and_then(|t| t.as_array())
                                {
                                    for tc in tcs {
                                        if let Some(idx) = tc.get("index").and_then(|i| i.as_u64())
                                        {
                                            let idx = idx as usize;
                                            let tracker = tool_calls.entry(idx).or_default();

                                            if let Some(id) =
                                                tc.get("id").and_then(|i| i.as_str())
                                            {
                                                tracker.id = id.to_string();
                                            }
                                            if let Some(func) = tc.get("function") {
                                                if let Some(name) =
                                                    func.get("name").and_then(|n| n.as_str())
                                                {
                                                    tracker.name = name.to_string();
                                                }
                                                if let Some(args) =
                                                    func.get("arguments").and_then(|a| a.as_str())
                                                {
                                                    tracker.arguments.push_str(args);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let s = e.to_string();
                    if s.contains("EOF") || s.contains("Stream ended") {
                        es.close();
                    } else {
                        eprintln!("\n\x1b[31m[Error: {}]\x1b[0m", s);
                        es.close();
                    }
                    break;
                }
            }
        }

        // Build the assistant message to append to history
        let mut assistant_msg = json!({ "role": "assistant" });
        if !full_response.is_empty() {
            assistant_msg["content"] = json!(full_response);
        }

        if !tool_calls.is_empty() {
            let mut tc_arr = vec![];
            let mut indices: Vec<&usize> = tool_calls.keys().collect();
            indices.sort(); // maintain execution order

            for &idx in &indices {
                let tc = &tool_calls[&idx];
                tc_arr.push(json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.name,
                        "arguments": tc.arguments
                    }
                }));
            }
            assistant_msg["tool_calls"] = json!(tc_arr);
        }

        // Append assistant's turnaround to our messages pipeline
        messages.push(assistant_msg.clone());

        // Also persist to current session history immediately, so we don't lose it if we loop or crash
        session.messages.push(assistant_msg.clone());
        save_session(&session);

        if tool_calls.is_empty() {
            // Reached final conversational answer. Break out.
            break 'chat;
        } else {
            // Agent wants to execute tools
            let mut indices: Vec<&usize> = tool_calls.keys().collect();
            indices.sort();

            let mut executed_results: HashMap<(&String, &String), String> = HashMap::new();

            for &idx in indices {
                let tc = &tool_calls[&idx];

                let result =
                    if let Some(cached) = executed_results.get(&(&tc.name, &tc.arguments)) {
                        cached.clone()
                    } else {
                        let r = execute_tool(&tc.name, &tc.arguments).await;
                        executed_results.insert((&tc.name, &tc.arguments), r.clone());
                        r
                    };

                let tool_msg = json!({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "content": result
                });

                messages.push(tool_msg.clone());

                session.messages.push(tool_msg);
                save_session(&session);
            }

            // Re-prompt the API with tools attached! Let's format nicely for UX:
            if !full_response.is_empty() && !full_response.ends_with('\n') {
                println!();
            }
        }
    }

    println!();

    Ok(())
}
