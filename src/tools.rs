use serde_json::{json, Value};
use std::fs;
use std::io::{self, Write};
use tokio::time::{timeout, Duration};
use tokio::process::Command as TokioCommand;

#[derive(Debug, Default)]
pub struct ToolCallTracker {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

pub fn get_tools() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "execute_cmd",
                "description": "Execute a terminal command. DO NOT run interactive apps, editors, or background servers. Only run commands that execute and terminate automatically.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The command to run, e.g., 'cargo check' or 'npm run build'"
                        }
                    },
                    "required": ["command"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read the contents of a file.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_dir",
                "description": "List files in a directory.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Path to the directory, e.g., '.' for current" }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Write or overwrite a file with new content. Can be used for small modifications.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "content": { "type": "string", "description": "The full file content to write" }
                    },
                    "required": ["path", "content"]
                }
            }
        }
    ])
}

pub async fn execute_tool(name: &str, args: &str) -> String {
    let parsed: Value = match serde_json::from_str(args) {
        Ok(v) => v,
        Err(e) => return format!("Tool argument parse error: {}", e),
    };

    match name {
        "execute_cmd" => {
            let cmd = parsed["command"].as_str().unwrap_or("");
            eprint!("\n\x1b[33m[Agent] Wants to run command:\x1b[0m {}\nAllow? [Y/n]: ", cmd);
            io::stdout().flush().unwrap();
            
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim().to_lowercase();
            if input == "n" || input == "no" {
                return "User denied permission to run the command.".into();
            }

            println!("\x1b[2mRunning command (max 60s timeout)...\x1b[0m");
            let is_windows = cfg!(target_os = "windows");
            let (exec_cmd, exec_args) = if is_windows {
                ("cmd", vec!["/C", cmd])
            } else {
                ("sh", vec!["-c", cmd])
            };

            let output_future = TokioCommand::new(exec_cmd)
                .args(&exec_args)
                .output();

            match timeout(Duration::from_secs(60), output_future).await {
                Ok(Ok(o)) => {
                    let mut res = format!("Exit status: {}\n", o.status);
                    if !o.stdout.is_empty() {
                        res.push_str(&format!("Stdout:\n{}\n", String::from_utf8_lossy(&o.stdout)));
                    }
                    if !o.stderr.is_empty() {
                        res.push_str(&format!("Stderr:\n{}\n", String::from_utf8_lossy(&o.stderr)));
                    }
                    
                    // Handle silent successes so the agent doesn't think it failed
                    if res.trim() == format!("Exit status: {}", o.status).trim() && o.status.success() {
                        res.push_str("(Command executed successfully with no output)");
                    }

                    if res.len() > 10000 {
                        res.truncate(10000);
                        res.push_str("\n...[output truncated]...");
                    }
                    res
                }
                Ok(Err(e)) => format!("Failed to spawn command: {}", e),
                Err(_) => "Error: Command timed out after 60 seconds. Do not run background servers or interactive commands.".into(),
            }
        }
        "read_file" => {
            let path = parsed["path"].as_str().unwrap_or("");
            println!("\x1b[2m[Agent] Reading file: {}\x1b[0m", path);
            match fs::read_to_string(path) {
                Ok(content) => content,
                Err(e) => format!("Failed to read file: {}", e),
            }
        }
        "list_dir" => {
            let path = parsed["path"].as_str().unwrap_or(".");
            println!("\x1b[2m[Agent] Listing dir: {}\x1b[0m", path);
            match fs::read_dir(path) {
                Ok(entries) => {
                    let mut res = String::new();
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        res.push_str(&format!("{}{}\n", name, if is_dir { "/" } else { "" }));
                    }
                    if res.is_empty() {
                        "[Empty directory]".into()
                    } else {
                        res
                    }
                }
                Err(e) => format!("Failed to list dir: {}", e),
            }
        }
        "write_file" => {
            let path = parsed["path"].as_str().unwrap_or("");
            let content = parsed["content"].as_str().unwrap_or("");
            eprint!("\n\x1b[33m[Agent] Wants to write to file:\x1b[0m {}\nAllow? [Y/n]: ", path);
            io::stdout().flush().unwrap();
            
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim().to_lowercase();
            if input == "n" || input == "no" {
                return "User denied permission to write the file.".into();
            }

            match fs::write(path, content) {
                Ok(_) => format!("Successfully wrote to {}", path),
                Err(e) => format!("Failed to write file: {}", e),
            }
        }
        _ => format!("Unknown tool: {}", name),
    }
}
