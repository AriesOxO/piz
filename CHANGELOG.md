# Changelog / 更新日志

All notable changes to this project will be documented in this file.
本文件记录项目的所有重要变更。

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).
格式基于 [Keep a Changelog](https://keepachangelog.com/)，版本号遵循[语义化版本](https://semver.org/)规范。

---

## [0.2.5] - 2026-03-17

### Added / 新增
- New injection detection patterns: `LD_PRELOAD` exploitation, `HISTFILE` suppression, process substitution redirection (`3<>`)
- 新增注入检测模式：`LD_PRELOAD` 利用、`HISTFILE` 抑制、进程替换重定向（`3<>`）
- Security policy (`SECURITY.md`), Code of Conduct, PR template for open-source governance
- 添加安全策略（`SECURITY.md`）、行为准则、PR 模板，完善开源治理
- CI: security audit (`cargo-audit`) and MSRV (1.70) verification
- CI：安全审计（`cargo-audit`）和最低 Rust 版本（1.70）验证

### Fixed / 修复
- Extracted common retry/error handling helpers in LLM backends, reducing code duplication
- 提取 LLM 后端通用重试/错误处理辅助函数，减少代码重复
- Improved error handling in chat history loading and update state (no longer silently swallows errors)
- 改善聊天历史加载和更新状态的错误处理（不再静默吞掉错误）
- Fixed `is_multiple_of` usage for MSRV 1.70 compatibility
- 修复 `is_multiple_of` 用法以兼容 MSRV 1.70
- Removed reference to non-existent `tests/manual/` directory in CONTRIBUTING.md
- 移除 CONTRIBUTING.md 中对不存在的 `tests/manual/` 目录的引用

## [0.2.3] - 2026-03-17

### Added / 新增
- Shell integration: `piz init <shell>` generates wrapper functions for bash/zsh/fish/PowerShell, enabling cd/export/source to work correctly in the current shell
- Shell 集成：`piz init <shell>` 生成 Shell 包装函数（bash/zsh/fish/PowerShell），使 cd/export/source 等命令在当前 Shell 中正确生效
- Eval mode (`--eval`): show UI normally, output confirmed command for shell wrapper to eval
- Eval 模式（`--eval`）：正常显示 UI，确认后将命令输出给 Shell 包装函数执行
- API-level JSON mode for OpenAI (`response_format`), Gemini (`responseMimeType`), Ollama (`format: json`)
- 启用 API 级 JSON 模式：OpenAI（`response_format`）、Gemini（`responseMimeType`）、Ollama（`format: json`）
- Structural regex fallback for broken JSON parsing (unescaped Windows path backslashes)
- 结构化正则回退解析，处理无效 JSON（Windows 路径反斜杠未转义问题）
- Manual test scripts for 25 complex scenarios (PowerShell + bash)
- 25 个复杂场景手动测试脚本（PowerShell + bash）

### Fixed / 修复
- Cache only after successful command execution, preventing failed commands from polluting cache
- 仅在命令成功执行后写入缓存，防止失败命令污染缓存
- Removed dangerous Level 4 raw-text-as-command fallback in response parsing
- 移除危险的第 4 级原始文本回退解析，防止 JSON 垃圾被当作命令执行
- Unified shell detection in executor with context module (parent-process based), replacing unreliable PSModulePath check
- 统一 executor 与 context 模块的 Shell 检测逻辑（基于父进程），替换不可靠的 PSModulePath 检查
- Non-invasive encoding: removed `[Console]::OutputEncoding` and `chcp 65001` prefixes, rely on GBK decode fallback
- 零侵入编码处理：移除 `[Console]::OutputEncoding` 和 `chcp 65001` 前缀，依赖 GBK 解码回退
- Added JSON backslash escaping instructions to all LLM prompts
- 在所有 LLM 提示词中添加 JSON 反斜杠转义说明

## [0.2.2] - 2026-03-17

### Added / 新增
- Self-update feature: `piz update` checks GitHub Releases, downloads and upgrades in-place with rollback on failure
- 自更新功能：`piz update` 检查 GitHub Releases，就地下载升级，失败自动回滚
- Background update check on each run (at most once per 24h), shows hint when new version available
- 每次运行后台检查更新（每 24 小时最多一次），有新版本时提示用户
- Two upgrade options: overwrite install or uninstall-then-reinstall, user chooses interactively
- 两种升级方式：覆盖安装或卸载后重装，用户交互式选择

### Fixed / 修复
- PowerShell: `Set-Location D:` goes to last visited path, not drive root; now generates `cd D:\` instead
- PowerShell：`Set-Location D:` 会跳转到上次访问路径而非盘符根目录，现在改为生成 `cd D:\`
- cmd.exe: added `cd /d` hint for cross-drive navigation
- cmd.exe：添加 `cd /d` 提示用于跨盘符切换

### Removed / 移除
- Japanese (ja) language support removed, now supports Chinese (zh) and English (en) only
- 移除日文（ja）语言支持，现仅支持中文（zh）和英文（en）

## [0.2.1] - 2026-03-17

### Fixed / 修复
- Windows shell detection: replaced `PSModulePath` env var check with WMIC parent process name detection, fixing cmd.exe being misidentified as PowerShell
- Windows Shell 检测：使用 WMIC 父进程名称检测替代 `PSModulePath` 环境变量检查，修复 cmd.exe 被错误识别为 PowerShell 的问题

## [0.2.0] - 2026-03-16

### Added / 新增
- Interactive chat mode (`piz chat`) with multi-turn context, `/help`, `/clear`, `/history` commands, and persistent history (`~/.piz/chat_history.json`)
- 交互式聊天模式（`piz chat`），支持多轮上下文、`/help`、`/clear`、`/history` 命令及持久化历史记录（`~/.piz/chat_history.json`）
- Multi-candidate command generation (`piz -n 3 list files`) with interactive selection
- 多候选命令生成（`piz -n 3 list files`），支持交互式选择
- Execution history tracking (`piz history`, `piz history <search> -l <limit>`)
- 执行历史记录（`piz history`、`piz history <search> -l <limit>`）
- Shell completion generation (`piz completions bash/zsh/fish/powershell`)
- Shell 补全脚本生成（`piz completions bash/zsh/fish/powershell`）
- Pipe mode (`piz --pipe`) for script-friendly output (command only, no UI)
- 管道模式（`piz --pipe`），输出纯命令文本，适合脚本集成
- Config management: `piz config --show` (API keys masked), `piz config --reset`
- 配置管理：`piz config --show`（API 密钥脱敏显示）、`piz config --reset`
- `--verbose` flag for debugging LLM prompts and responses
- `--verbose` 标志，用于调试 LLM 提示词和响应内容
- `NO_COLOR` environment variable support
- 支持 `NO_COLOR` 环境变量禁用彩色输出
- Cache LRU eviction with configurable `cache_max_entries` (default 1000)
- 缓存 LRU 淘汰策略，可配置 `cache_max_entries`（默认 1000）
- Cache expired entry cleanup on open
- 缓存启动时自动清理过期条目
- Injection detection on cached commands with automatic purge of poisoned entries
- 对缓存命令进行注入检测，自动清除被污染的条目
- New injection patterns: `curl -K` config file attack, `xargs rm`, `find -delete`, `find -exec rm`
- 新增注入检测模式：`curl -K` 配置文件攻击、`xargs rm`、`find -delete`、`find -exec rm`
- Injection detection messages internationalized (zh/en) via `InjectionReason` enum
- 注入检测消息国际化（中/英/日），通过 `InjectionReason` 枚举实现
- API retry with exponential backoff for 429/5xx errors (all backends)
- API 请求重试与指数退避，适用于 429/5xx 错误（所有后端）
- Unified `temperature` (0.1) and `max_tokens` (2048) across all LLM backends
- 统一所有 LLM 后端的 `temperature`（0.1）和 `max_tokens`（2048）参数
- Enhanced system context: architecture detection, git repo detection, package manager detection
- 增强系统上下文：CPU 架构检测、Git 仓库检测、包管理器检测
- Fish shell syntax hints in prompts
- 提示词中添加 Fish shell 语法提示
- PowerShell examples in prompts
- 提示词中添加 PowerShell 示例
- Auto-fix visual diff display (red strikethrough → green bold)
- 自动修复可视化差异显示（红色删除线 → 绿色加粗）
- Auto-fix retry loop for `piz fix` subcommand (up to 3 retries)
- `piz fix` 子命令自动修复重试循环（最多 3 次）
- Configurable `chat_history_size` in config
- 可配置 `chat_history_size` 聊天历史条数
- 157 tests (149 unit + 8 integration)
- 157 个测试（149 单元测试 + 8 集成测试）

### Changed / 变更
- Cache is now opened once per request instead of 3 times (performance improvement)
- 缓存每次请求仅打开一次，替代之前的 3 次（性能优化）
- `try_auto_fix()` moved from `main.rs` to `fix.rs` for better code organization
- `try_auto_fix()` 从 `main.rs` 移至 `fix.rs`，改善代码组织
- Danger level boundaries refined in prompt engineering
- 细化提示词中的危险等级判定边界

### Fixed / 修复
- Comprehensive code review fixes (security, bugs, robustness) from v0.1.1
- 全面代码审查修复（安全性、Bug、健壮性），基于 v0.1.1 版本

## [0.1.1] - 2026-03-16

### Fixed / 修复
- Windows console encoding (GBK garbled text) resolved
- 修复 Windows 控制台编码问题（GBK 乱码）
- Auto-fix on command failure with up to 3 retries
- 命令执行失败时自动修复，最多重试 3 次

## [0.1.0] - 2026-03-16

### Added / 新增
- Core natural language to shell command translation
- 核心功能：自然语言转 Shell 命令
- 4-level LLM response parsing fallback (JSON → embedded JSON → backtick → raw text)
- 4 级 LLM 响应解析回退（JSON → 内嵌 JSON → 反引号 → 纯文本）
- Multi-backend LLM support: OpenAI (with custom base_url), Claude, Gemini, Ollama
- 多后端 LLM 支持：OpenAI（可自定义 base_url）、Claude、Gemini、Ollama
- Dual danger detection: regex patterns + LLM classification
- 双重危险检测：正则表达式模式匹配 + LLM 分类
- Three danger levels: safe, warning, dangerous
- 三级危险等级：安全、警告、危险
- Command explain mode (`piz -e`)
- 命令解释模式（`piz -e`）
- Command fix mode (`piz fix`) with last_exec.json + shell history fallback
- 命令修复模式（`piz fix`），支持 last_exec.json + Shell 历史回退
- SQLite cache with SHA256 key and configurable TTL
- SQLite 缓存，SHA256 键值，可配置 TTL
- Interactive confirmation UI (Y/n/e) with editor support
- 交互式确认界面（Y/n/e），支持编辑器修改
- TOML configuration file (`~/.piz/config.toml`)
- TOML 配置文件（`~/.piz/config.toml`）
- Interactive configuration wizard with provider presets (OpenAI, DeepSeek, SiliconFlow, Moonshot, etc.)
- 交互式配置向导，内置供应商预设（OpenAI、DeepSeek、SiliconFlow、Moonshot 等）
- Multi-language UI support: Chinese (zh), English (en)
- 多语言 UI 支持：中文（zh）、英文（en）
- System context injection (OS, shell, cwd) into prompts
- 系统上下文注入提示词（操作系统、Shell、当前目录）
- Cross-platform support: Windows (PowerShell/cmd), macOS, Linux (bash/zsh)
- 跨平台支持：Windows（PowerShell/cmd）、macOS、Linux（bash/zsh）
- Prompt optimization: few-shot examples, shell-specific syntax hints, explicit language directives
- 提示词优化：few-shot 示例、Shell 特定语法提示、明确语言指令
