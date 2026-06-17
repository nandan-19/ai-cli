use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command as TokioCommand;
use serde_json::json;
use crate::session::{load_session, save_session};

pub async fn execute_rec(cmd_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
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
                    let _ = tx_out.send(String::from_utf8_lossy(&buf[..n]).into_owned());
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
                    let _ = tx_err.send(String::from_utf8_lossy(&buf[..n]).into_owned());
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
    Ok(())
}
