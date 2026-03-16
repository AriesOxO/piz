# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.2.0] - 2026-03-16

### Added
- Interactive chat mode (`piz chat`) with multi-turn context, `/help`, `/clear`, `/history` commands, and persistent history (`~/.piz/chat_history.json`)
- Multi-candidate command generation (`piz -n 3 list files`) with interactive selection
- Execution history tracking (`piz history`, `piz history <search> -l <limit>`)
- Shell completion generation (`piz completions bash/zsh/fish/powershell`)
- Pipe mode (`piz --pipe`) for script-friendly output (command only, no UI)
- Config management: `piz config --show` (API keys masked), `piz config --reset`
- `--verbose` flag for debugging LLM prompts and responses
- `NO_COLOR` environment variable support
- Cache LRU eviction with configurable `cache_max_entries` (default 1000)
- Cache expired entry cleanup on open
- Injection detection on cached commands with automatic purge of poisoned entries
- New injection patterns: `curl -K` config file attack, `xargs rm`, `find -delete`, `find -exec rm`
- Injection detection messages internationalized (zh/en/ja) via `InjectionReason` enum
- API retry with exponential backoff for 429/5xx errors (all backends)
- Unified `temperature` (0.1) and `max_tokens` (2048) across all LLM backends
- Enhanced system context: architecture detection, git repo detection, package manager detection
- Fish shell syntax hints in prompts
- PowerShell examples in prompts
- Auto-fix visual diff display (red strikethrough → green bold)
- Auto-fix retry loop for `piz fix` subcommand (up to 3 retries)
- Configurable `chat_history_size` in config
- 158 tests (150 unit + 8 integration)

### Changed
- Cache is now opened once per request instead of 3 times (performance improvement)
- `try_auto_fix()` moved from `main.rs` to `fix.rs` for better code organization
- Danger level boundaries refined in prompt engineering

### Fixed
- Comprehensive code review fixes (security, bugs, robustness) from v0.1.1

## [0.1.1] - 2026-03-16

### Fixed
- Windows console encoding (GBK garbled text) resolved
- Auto-fix on command failure with up to 3 retries

## [0.1.0] - 2026-03-16

### Added
- Core natural language to shell command translation
- 4-level LLM response parsing fallback (JSON → embedded JSON → backtick → raw text)
- Multi-backend LLM support: OpenAI (with custom base_url), Claude, Gemini, Ollama
- Dual danger detection: regex patterns + LLM classification
- Three danger levels: safe, warning, dangerous
- Command explain mode (`piz -e`)
- Command fix mode (`piz fix`) with last_exec.json + shell history fallback
- SQLite cache with SHA256 key and configurable TTL
- Interactive confirmation UI (Y/n/e) with editor support
- TOML configuration file (`~/.piz/config.toml`)
- Interactive configuration wizard with provider presets (OpenAI, DeepSeek, SiliconFlow, Moonshot, etc.)
- Multi-language UI support: Chinese (zh), English (en), Japanese (ja)
- System context injection (OS, shell, cwd) into prompts
- Cross-platform support: Windows (PowerShell/cmd), macOS, Linux (bash/zsh)
- Prompt optimization: few-shot examples, shell-specific syntax hints, explicit language directives
