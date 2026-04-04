<div align="center">

# AI CLI Assistant

[![Rust](https://img.shields.io/badge/rust-1.80%2B-blue.svg?style=flat-square)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-0.1.7-orange.svg?style=flat-square)](https://github.com/nandan-19/ai-cli/releases)
[![Groq](https://img.shields.io/badge/Powered%20by-Groq-f55a3c.svg?style=flat-square)](https://groq.com)
[![License](https://img.shields.io/badge/License-MIT-green.svg?style=flat-square)](LICENSE)

**A blazing fast, autonomous terminal companion powered by Groq.**  
Ask questions, run commands, generate commits, and render rich Markdown — all from your shell.

</div>


---

## Table of Contents

- [Features](#features)
- [Installation](#installation)
  - [Pre-built Binaries](#pre-built-binaries)
  - [Build from Source](#build-from-source)
- [Configuration](#configuration)
- [Command Reference](#command-reference)
  - [Direct Queries](#1-direct-queries)
  - [Follow-up with Context](#2-follow-up-with-context)
  - [Command Recording](#3-command-recording-rec)
  - [Auto Commit](#4-auto-commit-commit)
  - [Streaming Toggle](#5-streaming--markdown-mode-stream-toggle)
  - [Session Management](#6-session-management)
  - [Configuration Commands](#7-configuration-commands)
- [Output Modes](#output-modes)
- [Markdown Rendering](#markdown-rendering)
- [Security](#security)

<br>

The **AI CLI Assistant** integrates state-of-the-art LLMs directly into your terminal workflow. It operates not just as a chatbot, but as an **autonomous agent** capable of inspecting your environment, executing shell commands, reading and writing files, and managing sessions — all without leaving your shell.

<br>

## Features

- **Blazing Fast Streaming** — Groq's LPU infrastructure delivers near-instant token streaming.
- **Agentic Tool Calling** — The AI can list directories, read/write files, and run shell commands to solve tasks autonomously.
- **Rich Markdown Rendering** — Toggle to non-streaming mode for fully styled Markdown output with tables, code blocks, headings, lists, and more.
- **Isolated Sessions** — Conversation history is strictly bound to your terminal window/tab via process isolation.
- **Command Recording** — Pipe `stdout`/`stderr` of any shell command directly into the AI's context window.
- **Auto-Commit** — Analyzes your `git diff` and writes a conventional commit message for you.
- **Session Housekeeping** — Automatically cleans up orphaned session files from dead terminal processes.
- **Smart Error Handling** — Provides actionable guidance (e.g., auto-suggests context clear or model switch) when token/rate limits are reached.

<br>

## Installation

### Pre-built Binaries

Download the latest pre-compiled binary from the [**Releases**](https://github.com/nandan-19/ai-cli/releases/latest) page.

| Platform | Binary |
|---|---|
| **Linux** (x86-64) | `ai` |
| **Windows** (x86-64) | `ai.exe` |

**Linux / macOS:**
```bash
curl -Lo ai https://github.com/nandan-19/ai-cli/releases/latest/download/ai
chmod +x ai
sudo mv ai /usr/local/bin/ai
```

**Windows (PowerShell):**
```powershell
Invoke-WebRequest -Uri "https://github.com/nandan-19/ai-cli/releases/latest/download/ai.exe" -OutFile "ai.exe"
New-Item -ItemType Directory -Force -Path "C:\tools"
Move-Item -Path ".\ai.exe" -Destination "C:\tools\ai.exe" -Force
[Environment]::SetEnvironmentVariable('Path', $env:Path + ';C:\tools', 'User')
```

<br>

### Build from Source

**Prerequisites:** [Rust & Cargo](https://rustup.rs/)

```bash
git clone https://github.com/nandan-19/ai-cli.git
cd ai-cli
cargo build --release
```

**Install the binary:**

```bash
# Linux / macOS
sudo cp target/release/ai /usr/local/bin/ai

# Linux / macOS (no sudo)
mkdir -p ~/.local/bin && cp target/release/ai ~/.local/bin/ai

# Windows (PowerShell)
Copy-Item .\target\release\ai.exe C:\tools\ai.exe -Force
```

<br>

## Configuration

Before your first query, set your Groq API key:

```bash
ai set-key YOUR_GROQ_API_KEY
```

Optionally configure your preferred model, or run `ai set-model` without arguments to select from an interactive, dynamically fetched list of available Groq models:

```bash
ai set-model
ai set-model openai/gpt-oss-20b
```

> Get a free API key at [console.groq.com](https://console.groq.com). Recommended models: `openai/gpt-oss-20b`, `llama-3.3-70b-versatile`, `gemma2-9b-it`.

Config is stored at `~/.terminal_ai.json`.

<br>

## Command Reference

### 1. Direct Queries

Ask anything — no quotes required:

```bash
ai how do I reverse a string in Rust?
ai explain the difference between TCP and UDP
ai write a Python script to rename all JPGs in a folder
```

---

### 2. Follow-up with Context

Conversations in your terminal are automatically saved to your session history. You don't need any special flags to ask follow-up questions:

```bash
ai how do I parse JSON in Rust?
ai now make it handle errors with anyhow
ai add unit tests for that
```

---

### 3. Command Recording (`rec`)

Run any terminal command and capture its output (stdout + stderr) into the AI's context. The AI can then explain errors, suggest fixes, or continue from the output.

```bash
ai rec cargo build --release
ai why did it fail?

ai rec npm test
ai which test is slowest?

ai rec python script.py --verbose
```

> Commands run in your current shell. Output is stored in session history automatically.

---

### 4. Auto Commit (`commit`)

Analyzes your `git diff` and generates a conventional commit message, then commits automatically:

```bash
ai commit
```

- If changes are staged (`git add`), they are committed as-is.
- If nothing is staged, unstaged changes are committed using `git commit -a`.
- The AI reads the diff and writes a concise, descriptive commit message.

---

### 5. Streaming / Markdown Mode (`stream-toggle`)

Toggle between two output modes:

```bash
ai stream-toggle
```

| Mode | Behaviour | Header |
|---|---|---|
| **Streaming** (default) | Tokens print live as they arrive | `[model-name]` |
| **Markdown** | Waits for full response, renders styled output | `[model-name] · markdown` |

Run `ai stream-toggle` again to switch back.

---

### 6. Session Management

Sessions are isolated per terminal window. History is cleaned up automatically when terminals close.

| Command | Description |
|---|---|
| `ai history` | Print the full conversation history for this terminal window |
| `ai clear` | Wipe the conversation history for this terminal window |
| `ai clean-all` | Delete all orphaned session files from closed terminals |

---

### 7. Configuration Commands

| Command | Description |
|---|---|
| `ai set-key YOUR_API_KEY` | Save your Groq API key |
| `ai set-model [MODEL_ID]` | Set a specific model, or run without arguments for an interactive selection menu |


<br>

## Security

Granting an AI access to your terminal requires clear security boundaries:

- **Read operations** (list directories, read files) execute automatically.
- **Destructive operations** (write/overwrite files, run shell commands) pause and prompt `[Y/n]` before proceeding.

You remain in complete control of your machine at all times.

<div align="center">
  <br>
  <i>Built with Rust.</i>
</div>
