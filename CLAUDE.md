# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

piz is a Rust CLI tool that translates natural language into shell commands using LLM backends (OpenAI-compatible, Claude, Gemini, Ollama). It includes security layers (injection detection with i18n, danger classification), SQLite caching with LRU eviction, multi-language UI (zh/en), interactive chat mode, multi-candidate selection, execution history, shell completions, pipe mode, shell integration (`piz init`) with built-in aliases (`p`/`pf`/`pc`), eval mode (`--eval`), inline command explanation (`-d`/`--detail` and `show_explanation` config), non-invasive encoding (GBK decode fallback, no shell environment modification), auto-fix on command failure with retry, and Homebrew tap support (`brew install AriesOxO/tap/piz`).

## Build & Development Commands

```bash
cargo build                # Debug build
cargo build --release      # Release build
cargo test                 # Run all tests (437 tests: 344 unit + 45 integration + 48 windows shell)
cargo test <test_name>     # Run a single test by name
cargo fmt --all -- --check # Check formatting
cargo clippy -- -D warnings # Lint (CI treats warnings as errors)
```

Requires Rust 1.70+. On Windows: MinGW-w64 or MSVC toolchain.

## Architecture

**Entry flow:** `main.rs` parses CLI args (clap) -> dispatches to subcommands (fix, chat, config, clear-cache, explain, history, completions, init) or main translate flow -> calls LLM (with retry/backoff, API-level JSON mode) -> parses response (multi-level fallback: JSON > embedded JSON > structural regex > backtick) -> injection scan -> danger classification -> user prompt -> execute -> auto-fix on failure (up to 3 retries). Multi-candidate mode (`-n`) requests JSON array and presents selection UI. Eval mode (`--eval`) writes confirmed command to `~/.piz/eval_command` for shell wrapper to eval. `piz init <shell>` generates shell wrapper functions (bash/zsh/fish/PowerShell) enabling cd/export/source to work in the current shell. Detail mode (`-d`/`--detail` or `show_explanation` config) adds inline parameter-by-parameter explanation before the confirmation menu.

**LLM abstraction:** `src/llm/mod.rs` defines the `LlmBackend` trait with `chat()` and `chat_with_history()` methods. Four implementations: `openai.rs`, `claude.rs`, `gemini.rs`, `ollama.rs`. All backends have unified temperature (0.1), max_tokens (2048), and retry with exponential backoff for 429/5xx errors. Factory function `create_backend()` instantiates the correct backend from config. OpenAI backend also serves 12+ compatible providers via `base_url`.

**Security (3 layers):**

1. Prompt-level refusal — LLM returns `{"refuse": true}` for non-command input
2. Injection detection (`danger.rs`) — local regex scan with `InjectionReason` enum (12 variants), i18n messages, blocks malicious patterns. Cached commands are re-validated on retrieval.
3. Danger classification — regex patterns + LLM-provided level -> Safe/Warning/Dangerous

**Cache:** SQLite with SHA256 keys, configurable TTL, LRU eviction (`cache_max_entries`), expired entry cleanup on open. Also stores execution history for `piz history` subcommand. Explanation text is cached alongside commands with automatic schema migration for existing databases.

**Chat:** `src/chat.rs` — multi-turn interactive mode with `chat_with_history()`, slash commands (/help, /clear, /history, /detail), persistent history to `~/.piz/chat_history.json`.

**Config:** TOML at `~/.piz/config.toml`. Interactive setup wizard in `config.rs` with 12 provider presets. First run auto-triggers the wizard. Supports `--show` (masked keys, default), `--raw` (unmasked keys), and `--reset`. `--raw` takes priority over `--show`. `show_explanation` controls inline command explanation (default `false`).

## Key Conventions

- Commit messages: `<type>: <description>` where type is `feat`, `fix`, `refactor`, `test`, `docs`, `chore`
- CI runs on all three platforms (ubuntu, windows, macos)
- All tests must pass, clippy must be warning-free, code must be `cargo fmt` compliant

## Adding a New LLM Backend

1. Create `src/llm/your_backend.rs` implementing `LlmBackend` trait
2. Add retry loop using `super::should_retry()`, `super::backoff_delay()`, `super::MAX_RETRIES`
3. Use `super::DEFAULT_TEMPERATURE` and `super::DEFAULT_MAX_TOKENS` for consistency
4. Add config struct in `config.rs`
5. Register in `create_backend()` factory in `src/llm/mod.rs`
6. Add setup flow in `config.rs` init wizard

## Adding a New Language

1. Add variant to `Lang` enum in `src/i18n.rs`
2. Create a new static translation table (including all `inject_*` and `chat_*` fields)
3. Add match arm in `t()` function
4. Update language selector in `config.rs`

## Adding a New Injection Pattern

1. Add variant to `InjectionReason` enum in `src/danger.rs`
2. Add regex pattern in `detect_injection()` function
3. Add `inject_*` field to `T` struct in `src/i18n.rs` with translations for all languages (zh/en)
4. Implement `message()` match arm in `InjectionReason`
5. Add test case
