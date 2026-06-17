use crate::config::Config;
use reqwest::Client;
use serde_json::{Value, json};
use std::io::{self, Write};

pub async fn execute_commit(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let api_key = match &config.api_key {
        Some(k) => k.clone(),
        None => {
            println!("No API key set. Run:  ai set-key YOUR_KEY");
            return Ok(());
        }
    };

    println!("\x1b[2mAnalyzing git changes...\x1b[0m");

    // 1. Get staged changes first
    let output = std::process::Command::new("git")
        .args(["diff", "--staged"])
        .output()?;

    let mut diff = String::from_utf8_lossy(&output.stdout).to_string();
    let mut uses_all_flag = false;

    // 2. If nothing is staged, grab unstaged changes
    if diff.trim().is_empty() {
        let output_unstaged = std::process::Command::new("git").args(["diff"]).output()?;
        diff = String::from_utf8_lossy(&output_unstaged.stdout).to_string();

        if diff.trim().is_empty() {
            println!("No changes found in this repository.");
            return Ok(());
        }
        println!("\x1b[33mNote: Using unstaged changes.\x1b[0m");
        uses_all_flag = true; // We will use `git commit -a -m`
    }

    // Prevent blowing up the context window on massive refactors
    if diff.len() > 15000 {
        diff.truncate(15000);
        diff.push_str("\n...[diff truncated]...");
    }

    // 3. One-shot strict prompt to Groq
    let system_prompt = "You are an expert developer. Read the following git diff and write a concise, sensible commit message using the Conventional Commits format (e.g., feat:, fix:, refactor:, chore:). Do not include any other text, markdown blocks, quotes, or explanation. ONLY output the commit message itself.";

    let messages = vec![
        json!({ "role": "system", "content": system_prompt }),
        json!({ "role": "user", "content": diff }),
    ];

    let payload = json!({
        "model": config.model,
        "messages": messages,
    });

    let client = Client::new();
    let res = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await?;

    if !res.status().is_success() {
        eprintln!("API Error: {}", res.text().await?);
        return Ok(());
    }

    let data: Value = res.json().await?;
    let suggested_message = data["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim();

    // 4. Present to user for execution
    println!(
        "\n\x1b[36mSuggested Commit Message:\x1b[0m\n{}",
        suggested_message
    );

    print!("\nExecute this commit? [Y/n]: ");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim().to_lowercase();

    if input == "y" || input == "yes" || input == "" {
        let mut args = vec!["commit"];
        if uses_all_flag {
            args.push("-a"); // Automatically stage modified files
        }
        args.push("-m");
        args.push(suggested_message);

        let commit_out = std::process::Command::new("git").args(&args).output()?;

        if commit_out.status.success() {
            println!("✓ Successfully committed!");
            println!("{}", String::from_utf8_lossy(&commit_out.stdout).trim());
        } else {
            eprintln!(
                "\x1b[31mFailed to commit:\x1b[0m\n{}",
                String::from_utf8_lossy(&commit_out.stderr)
            );
        }
    } else {
        println!("Commit aborted.");
    }

    Ok(())
}
