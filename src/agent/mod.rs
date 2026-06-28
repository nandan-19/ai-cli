pub mod prompt;

use crate::config::Config;
use crate::markdown::render_markdown;
use crate::session::{Session, save_session};
use crate::tools::{ToolCallTracker, execute_tool, get_tools};
use futures::stream::StreamExt;
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
pub async fn run_chat_loop(
    prompt_text: &str,
    config: &Config,
    api_key: &str,
    session: &mut Session,
) -> Result<(), Box<dyn std::error::Error>> {
    let os_info = env::consts::OS;
    let arch_info = env::consts::ARCH;
    let cwd = env::current_dir().unwrap_or_default().display().to_string();

    let dynamic_system_prompt = format!(
        "{}\n\nCURRENT ENVIRONMENT:\n- Operating System: {}\n- Architecture: {}\n- Current Working Directory: {}",
        prompt::SYSTEM_PROMPT,
        os_info,
        arch_info,
        cwd
    );
    let mut messages: Vec<Value> = vec![json!({
        "role": "system",
        "content":dynamic_system_prompt
    })];

    // Always include full session history as context
    for msg in &session.messages {
        messages.push(msg.clone());
    }

    let user_msg = json!({ "role": "user", "content": prompt_text });
    messages.push(user_msg.clone());

    // Append new user query to the running session
    session.messages.push(user_msg);
    save_session(session);

    println!(
        "\n\x1b[36m[{}]{}\x1b[0m\n",
        config.model,
        if config.streaming { "" } else { " · markdown" }
    );

    let client = Client::new();

    'chat: loop {
        if config.streaming {
            // ── STREAMING branch ─────────────────────────────────────────────
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
                                    if let Some(text) =
                                        delta.get("content").and_then(|c| c.as_str())
                                    {
                                        print!("{}", text);
                                        io::stdout().flush().unwrap();
                                        full_response.push_str(text);
                                    }
                                    if let Some(tcs) =
                                        delta.get("tool_calls").and_then(|t| t.as_array())
                                    {
                                        for tc in tcs {
                                            if let Some(idx) =
                                                tc.get("index").and_then(|i| i.as_u64())
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
                                                    if let Some(args) = func
                                                        .get("arguments")
                                                        .and_then(|a| a.as_str())
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
                        // Try to parse a structured Groq error out of the SSE error string
                        if let Some(json_start) = s.find('{') {
                            if let Ok(v) = serde_json::from_str::<Value>(&s[json_start..]) {
                                let code = v["error"]["code"].as_str().unwrap_or("");
                                let msg = v["error"]["message"].as_str().unwrap_or(&s);
                                match code {
                                    "rate_limit_exceeded" => {
                                        eprintln!("\n\x1b[33m[Rate limit]\x1b[0m {}", msg);
                                        if msg.contains("reduce your message size")
                                            || msg.to_lowercase().contains("too large")
                                        {
                                            eprintln!(
                                                "\x1b[2mTip: Your session history may be too large for this model's token limits.\x1b[0m"
                                            );
                                            eprintln!(
                                                "\x1b[2mRun \x1b[0m\x1b[36mai clear\x1b[0m\x1b[2m to start a fresh session, or \x1b[0m\x1b[36mai set-model\x1b[0m\x1b[2m to switch to a model with higher limits.\x1b[0m"
                                            );
                                        }
                                    }
                                    "context_length_exceeded" => {
                                        eprintln!(
                                            "\n\x1b[33m[Context limit reached]\x1b[0m The conversation history is too long for this model."
                                        );
                                        eprintln!(
                                            "\x1b[2mRun \x1b[0m\x1b[36mai clear\x1b[0m\x1b[2m to start a fresh session and try again.\x1b[0m"
                                        );
                                    }
                                    _ => eprintln!("\n\x1b[31m[Error: {}]\x1b[0m", msg),
                                }
                                es.close();
                                break;
                            }
                        }
                        if !s.contains("EOF") && !s.contains("Stream ended") {
                            eprintln!("\n\x1b[31m[Error: {}]\x1b[0m", s);
                        }
                        es.close();
                        break;
                    }
                }
            }

            // Build + persist assistant message
            let mut assistant_msg = json!({ "role": "assistant" });
            if !full_response.is_empty() {
                assistant_msg["content"] = json!(full_response);
            }
            if !tool_calls.is_empty() {
                let mut tc_arr = vec![];
                let mut indices: Vec<&usize> = tool_calls.keys().collect();
                indices.sort();
                for &idx in &indices {
                    let tc = &tool_calls[&idx];
                    tc_arr.push(json!({
                        "id": tc.id,
                        "type": "function",
                        "function": { "name": tc.name, "arguments": tc.arguments }
                    }));
                }
                assistant_msg["tool_calls"] = json!(tc_arr);
            }
            messages.push(assistant_msg.clone());
            session.messages.push(assistant_msg.clone());
            save_session(session);

            if tool_calls.is_empty() {
                break 'chat;
            } else {
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
                    save_session(session);
                }
                if !full_response.is_empty() && !full_response.ends_with('\n') {
                    println!();
                }
            }
        } else {
            // ── NON-STREAMING branch (Markdown) ──────────────────────────────
            let payload = json!({
                "model": config.model,
                "messages": messages,
                "stream": false,
                "tools": get_tools(),
                "tool_choice": "auto"
            });

            let res = client
                .post("https://api.groq.com/openai/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&payload)
                .send()
                .await?;

            if !res.status().is_success() {
                let body = res.text().await?;
                // Parse Groq's structured error response
                if let Ok(v) = serde_json::from_str::<Value>(&body) {
                    let code = v["error"]["code"].as_str().unwrap_or("");
                    let msg = v["error"]["message"].as_str().unwrap_or(&body);
                    match code {
                        "rate_limit_exceeded" => {
                            eprintln!("\x1b[33m[Rate limit]\x1b[0m {}", msg);
                            if msg.contains("reduce your message size")
                                || msg.to_lowercase().contains("too large")
                            {
                                eprintln!(
                                    "\x1b[2mTip: Your session history may be too large for this model's token limits.\x1b[0m"
                                );
                                eprintln!(
                                    "\x1b[2mRun \x1b[0m\x1b[36mai clear\x1b[0m\x1b[2m to start a fresh session, or \x1b[0m\x1b[36mai set-model\x1b[0m\x1b[2m to switch to a model with higher limits.\x1b[0m"
                                );
                            }
                        }
                        "context_length_exceeded" => {
                            eprintln!(
                                "\x1b[33m[Context limit reached]\x1b[0m The conversation history is too long for this model."
                            );
                            eprintln!(
                                "\x1b[2mRun \x1b[0m\x1b[36mai clear\x1b[0m\x1b[2m to start a fresh session and try again.\x1b[0m"
                            );
                        }
                        _ => eprintln!("\x1b[31m[Error: {}]\x1b[0m", msg),
                    }
                } else {
                    eprintln!("\x1b[31m[Error: {}]\x1b[0m", body);
                }
                break;
            }

            let data: Value = res.json().await?;
            let choice = &data["choices"][0];
            let msg = &choice["message"];

            let full_response = msg["content"].as_str().unwrap_or("").to_string();
            let finish_reason = choice["finish_reason"].as_str().unwrap_or("");

            // Render Markdown if there is text content
            if !full_response.is_empty() {
                render_markdown(&full_response);
            }

            // Re-assemble tool_calls map from non-streaming response
            let mut tool_calls: HashMap<usize, ToolCallTracker> = HashMap::new();
            if let Some(tcs) = msg["tool_calls"].as_array() {
                for (idx, tc) in tcs.iter().enumerate() {
                    let mut tracker = ToolCallTracker::default();
                    tracker.id = tc["id"].as_str().unwrap_or("").to_string();
                    tracker.name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                    tracker.arguments = tc["function"]["arguments"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    tool_calls.insert(idx, tracker);
                }
            }

            // Build + persist assistant message
            let mut assistant_msg = json!({ "role": "assistant" });
            if !full_response.is_empty() {
                assistant_msg["content"] = json!(full_response);
            }
            if !tool_calls.is_empty() {
                let mut tc_arr = vec![];
                let mut indices: Vec<&usize> = tool_calls.keys().collect();
                indices.sort();
                for &idx in &indices {
                    let tc = &tool_calls[&idx];
                    tc_arr.push(json!({
                        "id": tc.id,
                        "type": "function",
                        "function": { "name": tc.name, "arguments": tc.arguments }
                    }));
                }
                assistant_msg["tool_calls"] = json!(tc_arr);
            }
            messages.push(assistant_msg.clone());
            session.messages.push(assistant_msg.clone());
            save_session(session);

            if finish_reason != "tool_calls" || tool_calls.is_empty() {
                break 'chat;
            } else {
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
                    save_session(session);
                }
            }
        }
    } // end 'chat

    println!("\n");
    Ok(())
}
