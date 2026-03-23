<div align="center">

# AI CLI Assistant

[![Rust](https://img.shields.io/badge/rust-1.80%2B-blue.svg?style=flat-square)](https://www.rust-lang.org)
[![Groq](https://img.shields.io/badge/Powered%20by-Groq-f55a3c.svg?style=flat-square)](https://groq.com)
[![License](https://img.shields.io/badge/License-MIT-green.svg?style=flat-square)](LICENSE)

**A blazing fast, autonomous terminal companion powered by Groq.**

</div>

---

## Table of Contents
- [Features](#features)
- [Installation](#installation)
  - [Pre-built Binaries](#pre-built-binaries)
  - [Prerequisites](#prerequisites)
  - [Build from Source](#build-from-source)
- [Configuration](#configuration)
- [Usage](#usage)
  - [Direct Queries](#1-direct-queries)
  - [Follow-up & Context](#2-follow-up--context--p----prev)
  - [Native Command Recording](#3-native-command-recording-rec)
  - [Session Management](#4-session-management)
- [Security](#security)

<br>

The **AI CLI Assistant** integrates state-of-the-art LLMs directly into your terminal workflow. It operates not just as a chatbot, but as an **agent** capable of securely inspecting your environment, executing shell commands, and managing files to solve problems autonomously.

<br>

## Features

- **Blazing Fast Output**: Leveraging Groq's high-speed LPU infrastructure for instant token streaming.
- **Agentic Tool Calling**: The AI natively integrates with your file system. It can seamlessly list directories, read/write files, and execute scripts to answer complex questions.
- **Isolated Sessions**: Conversation context is strictly bound to the specific terminal window or tab you are using.
- **Command Recording**: Automatically pipe the `stdout` and `stderr` of any terminal command directly into the AI's context window.
- **Frictionless Prompting**: No more quoting your terminal inputs. Ask questions in plain English directly from your shell prompt.

<br>

## Installation

### Pre-built Binaries

The easiest way to install is to download a pre-compiled binary directly from the [**Releases**](https://github.com/nandan-19/ai-cli/releases/latest) page.

| Platform | Download |
|---|---|
| **Linux** (x86-64) | `ai` |
| **Windows** (x86-64) | `ai.exe` |

**Linux:**
```bash
# Download the latest release
curl -Lo ai https://github.com/nandan-19/ai-cli/releases/latest/download/ai
chmod +x ai
sudo mv ai /usr/local/bin/ai
```

**Windows (PowerShell):**
```powershell
# Download the latest release
Invoke-WebRequest -Uri "https://github.com/nandan-19/ai-cli/releases/latest/download/ai.exe" -OutFile "ai.exe"
New-Item -ItemType Directory -Force -Path "C:\tools"
Move-Item -Path ".\ai.exe" -Destination "C:\tools\ai.exe" -Force
# Add to PATH permanently for the current user
[Environment]::SetEnvironmentVariable('Path', $env:Path + ';C:\tools', 'User')
```

<br>

### Prerequisites

Ensure you have [Rust & Cargo](https://rustup.rs/) installed on your machine.

### Build from Source

**1. Clone the repository**
```bash
git clone https://github.com/nandan-19/ai-cli.git
cd ai_cli
```

**2. Compile the binary**
```bash
cargo build --release
```

**3. Move to your PATH**

Depending on your operating system, move the resulting executable to a directory available in your system's `PATH`.

**Windows (PowerShell):**
```powershell
New-Item -ItemType Directory -Force -Path "C:\tools"
Copy-Item -Path ".\target\release\ai.exe" -Destination "C:\tools\ai.exe" -Force
$env:Path += ";C:\tools"
```

**macOS / Linux:**

Move the executable to `/usr/local/bin` (requires root privileges):
```bash
sudo cp target/release/ai /usr/local/bin/ai
```

*Alternatively, for a local user installation without `sudo`, you can move it to `~/.local/bin` and ensure that directory is added to your `PATH` in your `~/.bashrc` or `~/.zshrc`:*
```bash
mkdir -p ~/.local/bin
cp target/release/ai ~/.local/bin/ai
export PATH="$HOME/.local/bin:$PATH"
```

<br>

## Configuration

Before making your first query, you must authenticate the CLI using your Groq API key.

**1. Set the API Key**
```bash
ai set-key YOUR_GROQ_API_KEY
```

**2. Configure the Model (Optional)**
You can use any model supported by Groq. The CLI uses an optimized model by default, but you are free to configure it to your preference:
```bash
ai set-model YOUR_PREFERRED_MODEL_ID
```
*(e.g.,`openai/gpt-oss-20b`, `llama-3.3-70b-versatile`) 

<br>

## Usage

Interact with the assistant naturally. Quotation marks are completely optional.

### 1. Direct Queries
Ask questions or request code generation.
```bash
ai how do I parse JSON in Rust?
```

### 2. Follow-up & Context (`-p` / `--prev`)
Pass your terminal's most recent conversation history back to the AI for highly contextual follow-ups.
```bash
ai -p can you rewrite that using serde?
```

### 3. Native Command Recording (`rec`)
Use the `rec` subcommand to run any arbitrary terminal script. The AI will quietly execute it and cache the exit status, output, and errors directly into your session history.

```bash
ai rec npm run dev
```
If the command crashes, simply ask the AI to debug it:
```bash
ai -p why did the build fail?
```

### 4. Session Management

The AI isolates history per parent process. If you close your terminal, the session ends.

| Command | Description |
|---|---|
| `ai history` | Print the current terminal window's conversation history |
| `ai clear` | Wipe the context for the current terminal window |
| `ai clean-all` | Run a system-wide sweep to delete all orphaned background session files |

<br>

## Security

Granting an AI access to execute terminal commands requires strict security boundaries.

* **Read Operations** (like listing directories or reading file contents) are executed automatically if the AI requests them.
* **Destructive Operations** (like writing/overwriting files or executing shell commands) will immediately halt the agent and prompt you with a `[Y/n]` verification before proceeding. 

You remain in complete control of your host machine at all times.

<div align="center">
  <br>
  <i>Built with Rust.</i>
</div>
