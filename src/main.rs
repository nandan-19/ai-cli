mod cli;
mod config;
mod session;
mod tools;

use clap::Parser;
use futures::stream::StreamExt;
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command as TokioCommand;

use cli::{Cli, Commands};
use config::{load_config, save_config};
use session::{clean_orphaned_sessions, load_session, save_session, session_path};
use tools::{ToolCallTracker, execute_tool, get_tools};

/// Process inline Markdown spans into ANSI-escaped text.
/// Handles: **bold**, *italic*, ***bold italic***, `code`, ~~strikethrough~~,
///           [link](url), ![img](url), <autolink>, \escape
fn render_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Backslash escape
        if chars[i] == '\\' && i + 1 < len {
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }

        // Autolink  <url> or <email>
        if chars[i] == '<' {
            if let Some(end) = chars[i..].iter().position(|&c| c == '>') {
                let url: String = chars[i + 1..i + end].iter().collect();
                if url.contains("://") || url.contains('@') {
                    out.push_str(&format!("\x1b[4;34m{}\x1b[0m", url));
                    i += end + 1;
                    continue;
                }
            }
        }

        // Image  ![alt](url)
        if chars[i] == '!' && i + 1 < len && chars[i + 1] == '[' {
            if let Some(bracket_end) = chars[i + 2..].iter().position(|&c| c == ']') {
                let alt_end = i + 2 + bracket_end;
                if alt_end + 1 < len && chars[alt_end + 1] == '(' {
                    if let Some(paren_end) = chars[alt_end + 2..].iter().position(|&c| c == ')') {
                        let url: String =
                            chars[alt_end + 2..alt_end + 2 + paren_end].iter().collect();
                        let alt: String = chars[i + 2..alt_end].iter().collect();
                        out.push_str(&format!("\x1b[35m🖼 {}\x1b[0m \x1b[2m({})\x1b[0m", alt, url));
                        i = alt_end + 2 + paren_end + 1;
                        continue;
                    }
                }
            }
        }

        // Link  [text](url)
        if chars[i] == '[' {
            if let Some(bracket_end) = chars[i + 1..].iter().position(|&c| c == ']') {
                let text_end = i + 1 + bracket_end;
                if text_end + 1 < len && chars[text_end + 1] == '(' {
                    if let Some(paren_end) = chars[text_end + 2..].iter().position(|&c| c == ')') {
                        let url: String = chars[text_end + 2..text_end + 2 + paren_end]
                            .iter()
                            .collect();
                        let link_text: String = chars[i + 1..text_end].iter().collect();
                        let rendered_text = render_inline(&link_text);
                        out.push_str(&format!(
                            "\x1b[4;34m{}\x1b[0m \x1b[2m({})\x1b[0m",
                            rendered_text, url
                        ));
                        i = text_end + 2 + paren_end + 1;
                        continue;
                    }
                }
            }
        }

        // Code span  `code`  (may use multiple backticks)
        if chars[i] == '`' {
            let tick_start = i;
            while i < len && chars[i] == '`' {
                i += 1;
            }
            let tick_count = i - tick_start;
            let closing: String = std::iter::repeat('`').take(tick_count).collect();
            let rest: String = chars[i..].iter().collect();
            if let Some(end_pos) = rest.find(&closing) {
                let code = &rest[..end_pos];
                out.push_str(&format!("\x1b[96m{}\x1b[0m", code));
                i += end_pos + tick_count;
                continue;
            } else {
                // No closing — treat as literal
                for _ in 0..tick_count {
                    out.push('`');
                }
                continue;
            }
        }

        // ~~strikethrough~~
        if chars[i] == '~' && i + 1 < len && chars[i + 1] == '~' {
            i += 2;
            let mut word = String::new();
            while i < len {
                if chars[i] == '~' && i + 1 < len && chars[i + 1] == '~' {
                    i += 2;
                    break;
                }
                word.push(chars[i]);
                i += 1;
            }
            out.push_str(&format!("\x1b[9m{}\x1b[29m", word));
            continue;
        }

        // ==highlight==
        if chars[i] == '=' && i + 1 < len && chars[i + 1] == '=' {
            i += 2;
            let mut word = String::new();
            while i < len {
                if chars[i] == '=' && i + 1 < len && chars[i + 1] == '=' {
                    i += 2;
                    break;
                }
                word.push(chars[i]);
                i += 1;
            }
            out.push_str(&format!("\x1b[93;1m{}\x1b[0m", word));
            continue;
        }

        // ***bold italic***
        if chars[i] == '*' && i + 2 < len && chars[i + 1] == '*' && chars[i + 2] == '*' {
            i += 3;
            let mut word = String::new();
            while i < len {
                if chars[i] == '*' && i + 2 < len && chars[i + 1] == '*' && chars[i + 2] == '*' {
                    i += 3;
                    break;
                }
                word.push(chars[i]);
                i += 1;
            }
            out.push_str(&format!("\x1b[1;3m{}\x1b[0m", word));
            continue;
        }

        // **bold** or __bold__
        if (chars[i] == '*' && i + 1 < len && chars[i + 1] == '*')
            || (chars[i] == '_' && i + 1 < len && chars[i + 1] == '_')
        {
            let delim = chars[i];
            i += 2;
            let mut word = String::new();
            while i < len {
                if chars[i] == delim && i + 1 < len && chars[i + 1] == delim {
                    i += 2;
                    break;
                }
                word.push(chars[i]);
                i += 1;
            }
            out.push_str(&format!("\x1b[1m{}\x1b[22m", word));
            continue;
        }

        // *italic* or _italic_
        if chars[i] == '*' || chars[i] == '_' {
            let delim = chars[i];
            i += 1;
            let mut word = String::new();
            while i < len {
                if chars[i] == delim {
                    i += 1;
                    break;
                }
                word.push(chars[i]);
                i += 1;
            }
            out.push_str(&format!("\x1b[3m{}\x1b[23m", word));
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Render a full Markdown document to styled ANSI terminal output.
/// Covers: headings H1-H6, thematic breaks, fenced code blocks (``` and ~~~),
/// indented code blocks, blockquotes, ordered/unordered/task lists,
/// GFM tables, and all inline formatting via render_inline.
fn render_markdown(text: &str) {
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();
    let mut i = 0;

    // Terminal width for rulers (default 72 if unable to detect)
    let term_width: usize = 72;

    while i < total {
        let line = lines[i];
        let trimmed = line.trim();

        // ── Fenced code block ─────────────────────────────────────────────
        // Supports both ``` and ~~~
        let fence_char = if trimmed.starts_with("```") {
            Some('`')
        } else if trimmed.starts_with("~~~") {
            Some('~')
        } else {
            None
        };

        if let Some(fc) = fence_char {
            let fence_prefix: String = std::iter::repeat(fc).take(3).collect();
            let lang = trimmed.trim_start_matches(fc).trim();
            if !lang.is_empty() {
                println!("\x1b[2m[{}]\x1b[0m", lang);
            }
            i += 1;
            while i < total {
                let code_line = lines[i];
                if code_line.trim().starts_with(&fence_prefix) {
                    i += 1;
                    break;
                }
                println!(
                    "\x1b[48;5;235m\x1b[93m {:<width$}\x1b[0m",
                    code_line,
                    width = term_width.saturating_sub(1)
                );
                i += 1;
            }
            println!();
            continue;
        }

        // ── Indented code block (4+ spaces or 1 tab) ────────────────────
        if line.starts_with("    ") || line.starts_with('\t') {
            let code = if line.starts_with('\t') {
                &line[1..]
            } else {
                &line[4..]
            };
            println!(
                "\x1b[48;5;235m\x1b[93m {:<width$}\x1b[0m",
                code,
                width = term_width.saturating_sub(1)
            );
            i += 1;
            continue;
        }

        // ── Thematic break  --- / *** / ___ ─────────────────────────────
        let compact = trimmed.replace(' ', "");
        if compact == "---"
            || compact == "***"
            || compact == "___"
            || compact == "----"
            || compact == "====="
        {
            println!("\x1b[2m{}\x1b[0m", "─".repeat(term_width));
            i += 1;
            continue;
        }

        // ── ATX Headings  # through ###### ──────────────────────────────
        let heading_level = trimmed.chars().take_while(|&c| c == '#').count();
        if heading_level > 0
            && heading_level <= 6
            && trimmed.len() > heading_level
            && trimmed.as_bytes().get(heading_level) == Some(&b' ')
        {
            let content = &trimmed[heading_level + 1..];
            let rendered = render_inline(content);
            match heading_level {
                1 => {
                    let bar = "═".repeat(term_width);
                    println!("\x1b[1;38;5;220m{}\x1b[0m", bar);
                    println!("\x1b[1;38;5;220m  {}\x1b[0m", rendered);
                    println!("\x1b[1;38;5;220m{}\x1b[0m", bar);
                }
                2 => {
                    println!("\x1b[1;38;5;214m▌ {}\x1b[0m", rendered);
                    println!("\x1b[38;5;214m{}\x1b[0m", "─".repeat(term_width));
                }
                3 => println!("\x1b[1;38;5;208m◆ {}\x1b[0m", rendered),
                4 => println!("\x1b[1;38;5;203m● {}\x1b[0m", rendered),
                5 => println!("\x1b[1;38;5;198m○ {}\x1b[0m", rendered),
                6 => println!("\x1b[1;38;5;176m· {}\x1b[0m", rendered),
                _ => println!("{}", rendered),
            }
            i += 1;
            continue;
        }

        // ── Setext heading  (underlined with === or ---) ─────────────────
        if i + 1 < total {
            let next = lines[i + 1].trim();
            if !trimmed.is_empty() && (next.chars().all(|c| c == '=') && !next.is_empty()) {
                let rendered = render_inline(trimmed);
                let bar = "═".repeat(term_width);
                println!("\x1b[1;38;5;220m{}\x1b[0m", bar);
                println!("\x1b[1;38;5;220m  {}\x1b[0m", rendered);
                println!("\x1b[1;38;5;220m{}\x1b[0m", bar);
                i += 2;
                continue;
            }
            if !trimmed.is_empty() && (next.chars().all(|c| c == '-') && next.len() > 1) {
                let rendered = render_inline(trimmed);
                println!("\x1b[1;38;5;214m▌ {}\x1b[0m", rendered);
                println!("\x1b[38;5;214m{}\x1b[0m", "─".repeat(term_width));
                i += 2;
                continue;
            }
        }

        // ── Blockquote  > ... ────────────────────────────────────────────
        if trimmed.starts_with("> ") || trimmed == ">" {
            let content = if trimmed == ">" { "" } else { &trimmed[2..] };
            let rendered = render_inline(content);
            println!("\x1b[38;5;244m│\x1b[0m \x1b[3;38;5;252m{}\x1b[0m", rendered);
            i += 1;
            continue;
        }

        // ── GFM Table ────────────────────────────────────────────────────
        // Detect: line has | chars and next line is a separator row
        if trimmed.starts_with('|') || trimmed.contains(" | ") {
            // Peek ahead to see if next line is a separator (---|--- pattern)
            let is_table_header = i + 1 < total && {
                let sep = lines[i + 1].trim();
                sep.starts_with('|')
                    && sep.contains('-')
                    && sep.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
            };

            if is_table_header {
                // Collect all table rows
                let mut rows: Vec<Vec<String>> = Vec::new();
                let mut sep_idx = 0_usize;
                let mut j = i;
                while j < total {
                    let row = lines[j].trim();
                    if row.is_empty() {
                        break;
                    }
                    if row.starts_with('|') || row.contains('|') {
                        let is_sep = row.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '));
                        if is_sep {
                            sep_idx = rows.len();
                            rows.push(vec![]); // placeholder
                        } else {
                            let cells: Vec<String> = row
                                .split('|')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            rows.push(cells);
                        }
                    } else {
                        break;
                    }
                    j += 1;
                }

                // Compute column widths
                let col_count = rows
                    .iter()
                    .filter(|r| !r.is_empty())
                    .map(|r| r.len())
                    .max()
                    .unwrap_or(0);
                let mut col_widths = vec![3usize; col_count];
                for row in &rows {
                    for (ci, cell) in row.iter().enumerate() {
                        if ci < col_count {
                            col_widths[ci] = col_widths[ci].max(cell.len());
                        }
                    }
                }

                // Draw top border
                let top: String = col_widths
                    .iter()
                    .map(|&w| "─".repeat(w + 2))
                    .collect::<Vec<_>>()
                    .join("┬");
                println!("\x1b[38;5;244m┌{}┐\x1b[0m", top);

                let mut is_first = true;
                for (ri, row) in rows.iter().enumerate() {
                    if ri == sep_idx {
                        // separator row → draw mid-border
                        let mid: String = col_widths
                            .iter()
                            .map(|&w| "─".repeat(w + 2))
                            .collect::<Vec<_>>()
                            .join("┼");
                        println!("\x1b[38;5;244m├{}┤\x1b[0m", mid);
                        continue;
                    }
                    if row.is_empty() {
                        continue;
                    }

                    let row_str: String = row
                        .iter()
                        .enumerate()
                        .map(|(ci, cell)| {
                            let w = *col_widths.get(ci).unwrap_or(&3);
                            let rendered = render_inline(cell);
                            if is_first {
                                format!(" \x1b[1;36m{:<width$}\x1b[0m ", rendered, width = w)
                            } else {
                                format!(" {:<width$} ", rendered, width = w)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\x1b[38;5;244m│\x1b[0m");
                    println!("\x1b[38;5;244m│\x1b[0m{}\x1b[38;5;244m│\x1b[0m", row_str);
                    is_first = false;
                }

                // Draw bottom border
                let bot: String = col_widths
                    .iter()
                    .map(|&w| "─".repeat(w + 2))
                    .collect::<Vec<_>>()
                    .join("┴");
                println!("\x1b[38;5;244m└{}┘\x1b[0m", bot);
                println!();
                i = j;
                continue;
            }
        }

        // ── Task list  - [ ] / - [x] ─────────────────────────────────────
        if trimmed.starts_with("- [ ] ") || trimmed.starts_with("* [ ] ") {
            let content = render_inline(&trimmed[6..]);
            println!("  \x1b[38;5;244m☐\x1b[0m {}", content);
            i += 1;
            continue;
        }
        if trimmed.starts_with("- [x] ")
            || trimmed.starts_with("* [x] ")
            || trimmed.starts_with("- [X] ")
            || trimmed.starts_with("* [X] ")
        {
            let content = render_inline(&trimmed[6..]);
            println!("  \x1b[32m☑\x1b[0m \x1b[2m{}\x1b[0m", content);
            i += 1;
            continue;
        }

        // ── Unordered list  - / * / + ────────────────────────────────────
        let indent_spaces = line.len() - line.trim_start().len();
        let is_unordered =
            trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ");
        if is_unordered {
            let content = render_inline(&trimmed[2..]);
            let indent = "  ".repeat(indent_spaces / 2);
            let bullet = match indent_spaces / 2 {
                0 => "\x1b[36m•\x1b[0m",
                1 => "\x1b[35m◦\x1b[0m",
                _ => "\x1b[34m▸\x1b[0m",
            };
            println!("{}  {} {}", indent, bullet, content);
            i += 1;
            continue;
        }

        // ── Ordered list  1. 2. etc ─────────────────────────────────────
        let is_ordered = {
            let mut dot_pos = None;
            for (ci, ch) in trimmed.char_indices() {
                if ch == '.' || ch == ')' {
                    dot_pos = Some(ci);
                    break;
                }
                if !ch.is_ascii_digit() {
                    break;
                }
            }
            dot_pos
                .map(|dp| trimmed[..dp].parse::<u32>().is_ok() && trimmed.len() > dp + 1)
                .unwrap_or(false)
        };
        if is_ordered {
            let dot_pos = trimmed.find('.').or_else(|| trimmed.find(')')).unwrap_or(0);
            let num = &trimmed[..dot_pos];
            let content = render_inline(trimmed[dot_pos + 1..].trim_start());
            let indent = "  ".repeat(indent_spaces / 2);
            println!("{}  \x1b[1;36m{}.\x1b[0m {}", indent, num, content);
            i += 1;
            continue;
        }

        // ── Blank line ───────────────────────────────────────────────────
        if trimmed.is_empty() {
            println!();
            i += 1;
            continue;
        }

        // ── Plain paragraph ──────────────────────────────────────────────
        println!("{}", render_inline(trimmed));
        i += 1;
    }
    println!();
}

const SYSTEM_PROMPT: &str =
    "You are a deterministic CLI AI agent operating inside a terminal environment.

PRIMARY OBJECTIVE:
Execute user intent with maximum correctness, minimal assumptions, and zero unnecessary actions.

OPERATING PRINCIPLES:

1. DETERMINISM
- Do not guess. Do not hallucinate.
- If information is missing, explicitly ask for it.
- Every action must be justified by the user request or observed system state.

2. TOOL USAGE POLICY
- Tools represent real system actions (shell commands, file ops).
- Only call a tool if:
  a) It is REQUIRED to progress the task
  b) The expected outcome is known and useful
- Never call tools speculatively.
- Never repeat a tool call with identical arguments after failure.

3. EXECUTION MODEL
- Think step-by-step before acting:
  a) Understand intent
  b) Validate constraints (OS, files, permissions, dependencies)
  c) Decide minimal action
- Prefer the smallest valid command over complex pipelines.

4. ERROR HANDLING
- On failure:
  a) Parse the error message
  b) Identify root cause (missing file, permission, syntax, dependency)
  c) Modify strategy
- If 2 attempts fail → STOP and ask user for clarification.

5. COMMAND SAFETY
- NEVER run:
  - Interactive commands (vim, nano, less, top)
  - Long-running processes (servers, watchers)
- All commands must terminate quickly.

6. STATE AWARENESS
- Track:
  - What has been executed
  - What succeeded
  - What failed
- Do not redo successful steps.

7. OUTPUT CONTRACT
- Be concise and terminal-friendly.
- No markdown explanations unless necessary.
- No verbosity, no storytelling.
- If using tools → prioritize execution over explanation.

8. EDGE CASE HANDLING
- If multiple valid approaches exist:
  - Choose the simplest
  - Mention alternatives only if relevant
- If environment is ambiguous → ask before acting.

9. USER OVERRIDE
- If user explicitly requests something unsafe or inefficient:
  - Warn briefly
  - Still comply unless destructive

FAILURE CONDITIONS (STOP IMMEDIATELY):
- Missing critical information
- Repeated command failure
- Ambiguous user intent

Your behavior must resemble a careful systems engineer, not a conversational assistant.";

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
            Commands::Commit => {
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
                    let output_unstaged =
                        std::process::Command::new("git").args(["diff"]).output()?;
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
            }
            Commands::SetKey { key } => {
                config.api_key = Some(key);
                save_config(&config);
                println!("✓ API key saved.\n");
            }
            Commands::SetModel { model } => {
                config.model = model.clone();
                save_config(&config);
                println!("✓ Model switched to {}.\n", model);
            }
            Commands::History => {
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
            }
            Commands::Clear => {
                let p = session_path();
                if p.exists() {
                    fs::remove_file(&p).ok();
                    println!("✓ Session history cleared.");
                } else {
                    println!("No history to clear.\n");
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
                println!("✓ Cleared {} session(s) across all terminals.\n", count);
            }
            Commands::StreamToggle => {
                config.streaming = !config.streaming;
                save_config(&config);
                let mode = if config.streaming {
                    "\x1b[32mstreaming\x1b[0m (live output)"
                } else {
                    "\x1b[35mnon-streaming\x1b[0m (rendered Markdown)"
                };
                println!("✓ Switched to {} mode.", mode);
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
        println!(
            "\n\x1b[1;36mai\x1b[0m  \x1b[2mv{}\x1b[0m — terminal AI assistant powered by Groq\n",
            env!("CARGO_PKG_VERSION")
        );
        println!("\x1b[1mUsage\x1b[0m");
        println!("  ai \x1b[36m<question>\x1b[0m              ask anything, no quotes needed");
        println!(
            "  ai \x1b[36m-p <question>\x1b[0m           follow-up using session history as context"
        );
        println!();
        println!("\x1b[1mSubcommands\x1b[0m");
        println!(
            "  ai \x1b[36mrec\x1b[0m \x1b[2m<cmd>\x1b[0m               run a command and capture output into history"
        );
        println!(
            "  ai \x1b[36mcommit\x1b[0m                  analyze git diff and auto-generate a commit"
        );
        println!(
            "  ai \x1b[36mstream-toggle\x1b[0m           switch between live streaming and markdown mode"
        );
        println!(
            "  ai \x1b[36mhistory\x1b[0m                 print this session's conversation history"
        );
        println!(
            "  ai \x1b[36mclear\x1b[0m                   wipe this session's conversation history"
        );
        println!(
            "  ai \x1b[36mclean-all\x1b[0m               remove orphaned session files from closed terminals"
        );
        println!(
            "  ai \x1b[36mset-key\x1b[0m \x1b[2m<key>\x1b[0m            save your Groq API key"
        );
        println!(
            "  ai \x1b[36mset-model\x1b[0m \x1b[2m<model>\x1b[0m         set the model for all queries"
        );
        println!();
        println!("\x1b[2mRun `ai <subcommand> --help` for details on any command.\x1b[0m\n");
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

    println!(
        "\n\x1b[36m[{}]{}\x1b[0m\n",
        config.model,
        if config.streaming { "" } else { " · markdown" }
    );

    // ── Agent Loop ───────────────────────────────────────────────────────────

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
            save_session(&session);

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
                    save_session(&session);
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
                eprintln!("\x1b[31m[Error: {}]\x1b[0m", res.text().await?);
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
            save_session(&session);

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
                    save_session(&session);
                }
            }
        }
    } // end 'chat

    println!("\n");

    Ok(())
}
