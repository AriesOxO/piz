# Contributing to piz

Thank you for your interest in contributing to piz! This document provides guidelines and information for contributors.

## Getting Started

1. **Fork** the repository on GitHub
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/piz.git
   cd piz
   ```
3. **Create a branch** for your changes:
   ```bash
   git checkout -b feat/your-feature-name
   ```
4. **Build and test**:
   ```bash
   cargo build
   cargo test
   ```

## Development Setup

### Prerequisites

- Rust 1.70 or later
- On Windows: MinGW-w64 toolchain (for `windows-gnu` target) or MSVC

### Build

```bash
cargo build            # Debug build
cargo build --release  # Release build
cargo test             # Run all tests (158 tests)
cargo fmt --all -- --check  # Check formatting
cargo clippy -- -D warnings # Lint check
```

## How to Contribute

### Reporting Bugs

Open an [issue](https://github.com/AriesOxO/piz/issues/new) with:
- piz version (`piz --version`)
- OS and shell
- Steps to reproduce
- Expected vs actual behavior

### Suggesting Features

Open an [issue](https://github.com/AriesOxO/piz/issues/new) describing:
- The problem you're trying to solve
- Your proposed solution
- Any alternatives you considered

### Pull Requests

1. Ensure your code builds without warnings: `cargo build`
2. All tests pass: `cargo test`
3. Format your code: `cargo fmt`
4. Run clippy: `cargo clippy -- -D warnings`
5. Write tests for new functionality
6. Keep commits focused — one logical change per commit

#### Commit Message Format

```
<type>: <short description>

<optional body>
```

Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`

Examples:
- `feat: add Gemini backend support`
- `fix: handle empty LLM response gracefully`
- `docs: add SiliconFlow config example`

### Areas for Contribution

- **New LLM backends** — Add support for more providers
- **Danger patterns** — Expand regex detection rules in `danger.rs`
- **Injection patterns** — Add new `InjectionReason` variants with i18n messages
- **i18n** — Add new languages or improve translations
- **Platform support** — Improve Windows/macOS compatibility
- **Tests** — Increase coverage, especially edge cases
- **Documentation** — Improve README, add examples

## Project Structure

```
src/
├── main.rs          # Entry point, CLI dispatch, response parsing, multi-candidate selection
├── cli.rs           # clap argument definitions (with clap_complete)
├── config.rs        # Config loading + setup wizard (12 provider presets)
├── context.rs       # System context (OS, shell, cwd, arch, git, package manager)
├── i18n.rs          # UI translations (zh/en/ja) including injection messages
├── llm/
│   ├── mod.rs       # LlmBackend trait + factory + retry/backoff
│   ├── prompt.rs    # Prompt templates (translate, fix, explain, chat, multi-candidate)
│   ├── openai.rs    # OpenAI adapter (with retry)
│   ├── claude.rs    # Claude adapter (with retry)
│   ├── gemini.rs    # Gemini adapter (with retry)
│   └── ollama.rs    # Ollama adapter (with retry)
├── cache.rs         # SQLite cache (TTL + LRU eviction) + execution history
├── danger.rs        # Danger detection + injection scanner (InjectionReason enum)
├── executor.rs      # Command execution + user confirmation
├── explain.rs       # Explain mode
├── fix.rs           # Fix mode + auto-fix retry loop
├── chat.rs          # Interactive chat mode (slash commands + persistent history)
├── history.rs       # Shell history reader
└── ui.rs            # Terminal output (spinner, diff, colors)
```

### Adding a New LLM Backend

1. Create `src/llm/your_backend.rs`
2. Implement the `LlmBackend` trait (`chat()` and `chat_with_history()`)
3. Add retry loop using `super::should_retry()`, `super::backoff_delay()`, `super::MAX_RETRIES`
4. Use `super::DEFAULT_TEMPERATURE` and `super::DEFAULT_MAX_TOKENS`
5. Add config struct in `config.rs`
6. Register in factory function `create_backend()` in `src/llm/mod.rs`
7. Add setup flow in `config.rs` init wizard
8. Write tests

### Adding a New Language

1. Add a variant to `Lang` enum in `src/i18n.rs`
2. Create a new `static` translation table (including all `inject_*`, `chat_*`, and `select_command` fields)
3. Add the match arm in `t()` function
4. Update the language selector in `config.rs`

### Adding a New Injection Pattern

1. Add a variant to `InjectionReason` enum in `src/danger.rs`
2. Add regex pattern tuple in `detect_injection()` patterns list
3. Add `inject_*` field to `T` struct in `src/i18n.rs`
4. Add translations for all 3 languages (zh, en, ja)
5. Add match arm in `InjectionReason::message()`
6. Add test case in `danger.rs` tests
7. Update `all_langs_have_translations` test in `i18n.rs`

## Code of Conduct

- Be respectful and constructive
- Focus on the code, not the person
- Welcome newcomers and help them get started

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
