use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "ai",
    about = "A blazing fast, autonomous terminal AI assistant powered by Groq",
    long_about = "A blazing fast, autonomous terminal AI assistant powered by Groq.\n\
\n\
Ask questions in plain English — no quotes needed:\n\
  ai how do I reverse a string in Rust?\n\
  ai explain the OSI model\n\
\n\
Use -p / --prev to include your last conversation as context:\n\
  ai -p rewrite that using anyhow\n\
\n\
Use subcommands for sessions, config, recording, commits, and output modes.\n\
Run `ai <subcommand> --help` for details on any command.",
    version,
    arg_required_else_help = false
)]
pub struct Cli {
    /// The question or instruction to send to the AI (no quotes needed)
    #[arg(num_args = 0..)]
    pub prompt: Vec<String>,

    /// Include the current session's conversation history as context
    #[arg(
        short = 'p',
        long = "prev",
        help = "Include session history as context for follow-up questions"
    )]
    pub prev_context: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Save your Groq API key to the config file (~/.terminal_ai.json)
    #[command(
        name = "set-key",
        about = "Save your Groq API key",
        long_about = "Save your Groq API key to the local config file (~/.terminal_ai.json).\n\
\n\
Get a free key at https://console.groq.com\n\
\n\
Example:\n\
  ai set-key gsk_xxxxxxxxxxxxxxxxxxxx"
    )]
    SetKey { key: String },

    /// Set the model used for all queries
    #[command(
        name = "set-model",
        about = "Set the Groq model to use",
        long_about = "Set the Groq model used for all queries. The model ID is saved to\n\
~/.terminal_ai.json and persists across sessions.\n\
\n\
Recommended models:\n\
  openai/gpt-oss-20b        (fast, balanced)\n\
  llama-3.3-70b-versatile   (high quality)\n\
  gemma2-9b-it              (lightweight)\n\
\n\
Example:\n\
  ai set-model openai/gpt-oss-20b"
    )]
    SetModel { model: String },

    /// Print the conversation history for this terminal window
    #[command(
        name = "history",
        about = "Print the current session's conversation history",
        long_about = "Print all messages in the conversation history for this terminal window.\n\
\n\
Sessions are isolated per terminal. Closing a terminal ends its session.\n\
Use `ai clear` to wipe history without closing the terminal."
    )]
    History,

    /// Wipe the conversation history for this terminal window
    #[command(
        name = "clear",
        about = "Clear the current session's conversation history",
        long_about = "Delete all messages in the conversation history for this terminal window.\n\
\n\
This does not affect other terminal windows. To remove orphaned session\n\
files from closed terminals, use `ai clean-all`."
    )]
    Clear,

    /// Analyze git diff and generate a commit message automatically
    #[command(
        name = "commit",
        about = "Analyze git diff and auto-generate a commit message",
        long_about = "Reads your current git diff and uses the AI to generate a concise,\n\
conventional commit message, then commits automatically.\n\
\n\
Behavior:\n\
  - Staged changes (git add) are committed as-is\n\
  - If nothing is staged, unstaged changes are committed with `git commit -a`\n\
  - If there are no changes at all, the command exits with a message\n\
\n\
Example:\n\
  git add src/main.rs\n\
  ai commit"
    )]
    Commit,

    /// Delete all orphaned session files from closed terminals
    #[command(
        name = "clean-all",
        about = "Remove orphaned session files from closed terminals",
        long_about = "Scans for and deletes session files belonging to terminal processes\n\
that are no longer running.\n\
\n\
This is run automatically in the background on each invocation, but\n\
you can trigger it manually if needed."
    )]
    CleanAll,

    /// Run a command and capture its output into the AI session history
    #[command(
        name = "rec",
        about = "Record a command's output into session history",
        long_about = "Run any shell command and capture its stdout and stderr directly into\n\
the current session's conversation history. The AI can then analyze\n\
the output on a follow-up query.\n\
\n\
Examples:\n\
  ai rec cargo build --release\n\
  ai -p why did the build fail?\n\
\n\
  ai rec npm test\n\
  ai -p which test took the longest?\n\
\n\
  ai rec python script.py --verbose"
    )]
    Rec {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        cmd_args: Vec<String>,
    },

    /// Toggle between live streaming and Markdown rendering mode
    #[command(
        name = "stream-toggle",
        about = "Toggle between streaming and Markdown output mode",
        long_about = "Switch the output mode between:\n\
\n\
  Streaming (default)\n\
    Tokens are printed live as they arrive from the API.\n\
    Best for conversational use and long answers.\n\
    Header: [model-name]\n\
\n\
  Markdown\n\
    Waits for the full response, then renders it with ANSI styling.\n\
    Renders: headings, bold, italic, code blocks, tables, lists,\n\
    blockquotes, task lists, strikethrough, links, and more.\n\
    Header: [model-name] · markdown\n\
\n\
Run `ai stream-toggle` again to switch back."
    )]
    StreamToggle,
}
