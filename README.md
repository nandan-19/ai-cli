# AI CLI Assistant

A powerful, native command-line interface AI assistant powered by Groq and Llama 3 models. Designed for speed, utility, and seamless integration into your daily terminal workflow.

The assistant is context-aware and agentic. It can naturally converse with you, retain history per terminal session, execute shell commands, read and write files, and explore your local directories to accomplish tasks autonomously.

## Features

- **Blazing Fast**: Powered by Groq's high-speed inference engine.
- **Agentic Tool Calling**: The AI can execute local commands, read files, list directories, and write code directly to your machine (with your explicit permission for destructive actions).
- **Session Isolation**: Each terminal window or tab gets its own isolated conversation history.
- **Command Recording**: Run a command through the AI to automatically capture its output or errors into the context window for troubleshooting.
- **Quote-less Prompts**: No need to wrap your questions in quotes. Just type naturally.

## Installation & Setup

### Requirements

- [Rust & Cargo](https://rustup.rs/)

### Build from Source

1. Clone the repository and navigate into the project directory:
   ```bash
   git clone <your-repo-url>
   cd ai_cli
   ```

2. Build the project using Cargo:
   ```bash
   cargo build --release
   ```

3. Make the executable globally available:
   Move the compiled binary from `target/release/ai.exe` (Windows) or `target/release/ai` (Unix) to a directory in your system's `PATH`.

   For Windows (PowerShell):
   ```powershell
   Copy-Item -Path ".\target\release\ai.exe" -Destination "C:\tools\ai.exe" -Force
   $env:Path += ";C:\tools"
   ```

### Configuration

Before using the CLI, you must configure it with your Groq API key.

1. Set your Groq API key:
   ```bash
   ai set-key YOUR_GROQ_API_KEY
   ```

2. (Optional) Change the default model. The default is `llama-3.3-70b-versatile`.
   ```bash
   ai set-model llama-3.1-8b-instant
   ```

## Usage

Interact with the assistant naturally directly from your prompt.

### Basic Prompting
Ask any question without quotes:
```bash
ai what is the best rust web framework?
```

### Contextual Prompting
Use the `-p` or `--prev` flag to pass the recent session history as context to the AI. This is useful for follow-up questions or debugging previous command outputs.
```bash
ai -p can you explain that in more detail?
```

### Command Recording
Prefix shell commands with `ai rec` to execute them natively and automatically capture their stdout/stderr directly into the AI's context.
```bash
ai rec npm run dev
```
If the command fails, you can immediately ask the AI to debug it:
```bash
ai -p why did that fail?
```

### Session Management

The AI maintains isolated history files per terminal session.

- **View History**: See the context trace for your current terminal session.
  ```bash
  ai history
  ```

- **Clear History**: Wipe the context for the current terminal session.
  ```bash
  ai clear
  ```

- **Clean Orphaned Sessions**: Clear all background cache files globally across the system. (Useful if you have many closed terminals).
  ```bash
  ai clean-all
  ```

## Security & Privacy 

The AI agent has the ability to write to files and execute shell commands. To maintain system security:
- **Read-only tools** (listing directories, reading files) execute automatically if the AI requests them.
- **Destructive tools** (writing files, executing shell commands) will always pause execution and prompt you for an explicit `[Y/n]` confirmation before proceeding.
