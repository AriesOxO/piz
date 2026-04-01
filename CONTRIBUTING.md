# 贡献指南

[English](./CONTRIBUTING_EN.md) | 简体中文

感谢你对 piz 项目的关注！本文档提供贡献者指南和相关信息。

## 开始

1. 在 GitHub 上 **Fork** 本仓库
2. **Clone** 你的 fork：
   ```bash
   git clone https://github.com/YOUR_USERNAME/piz.git
   cd piz
   ```
3. **创建分支**：
   ```bash
   git checkout -b feat/your-feature-name
   ```
4. **构建和测试**：
   ```bash
   cargo build
   cargo test
   ```

## 开发环境

### 前提条件

- Rust 1.70 或更高版本
- Windows 系统需要 MinGW-w64 工具链（`windows-gnu` target）或 MSVC

### 构建

```bash
cargo build            # Debug 构建
cargo build --release  # Release 构建
cargo test             # 运行所有测试（437 个）
cargo fmt --all -- --check  # 检查格式
cargo clippy -- -D warnings # Lint 检查
```

## 如何贡献

### 报告 Bug

提交 [Issue](https://github.com/AriesOxO/piz/issues/new)，请包含：
- piz 版本（`piz --version`）
- 操作系统和 Shell 类型
- 复现步骤
- 预期行为和实际行为

### 功能建议

提交 [Issue](https://github.com/AriesOxO/piz/issues/new)，请描述：
- 你要解决的问题
- 你建议的解决方案
- 你考虑过的替代方案

### Pull Request

1. 确保代码无警告：`cargo build`
2. 所有测试通过：`cargo test`
3. 格式化代码：`cargo fmt`
4. 运行 clippy：`cargo clippy -- -D warnings`
5. 为新功能编写测试
6. 保持 commit 粒度清晰 —— 每个 commit 一个逻辑变更

#### Commit 消息格式

```
<type>: <简短描述>

<可选正文>
```

类型：`feat`、`fix`、`refactor`、`test`、`docs`、`chore`

示例：
- `feat: 添加 Gemini 后端支持`
- `fix: 优雅处理空的 LLM 响应`
- `docs: 添加硅基流动配置示例`

### 贡献方向

- **新 LLM 后端** — 添加更多供应商支持
- **危险模式** — 扩展 `danger.rs` 中的正则检测规则
- **注入模式** — 添加新的 `InjectionReason` 变体及 i18n 消息
- **国际化** — 添加新语言或改进翻译
- **平台支持** — 提升 Windows/macOS 兼容性
- **测试** — 提高覆盖率，尤其是边界情况
- **文档** — 改进 README，添加使用示例

## 项目结构

```
src/
├── main.rs          # 入口，CLI 分发，响应解析，多候选选择
├── cli.rs           # clap 命令行参数定义（含 clap_complete）
├── config.rs        # 配置加载 + 配置向导（12 个供应商预设）
├── context.rs       # 系统上下文（OS、Shell、CWD、架构、Git、包管理器）
├── i18n.rs          # UI 翻译（中/英），含注入检测消息
├── llm/
│   ├── mod.rs       # LlmBackend trait + 工厂函数 + 重试/退避
│   ├── prompt.rs    # Prompt 模板（翻译、修复、解释、对话、多候选）
│   ├── openai.rs    # OpenAI 适配器（含重试）
│   ├── claude.rs    # Claude 适配器（含重试）
│   ├── gemini.rs    # Gemini 适配器（含重试）
│   └── ollama.rs    # Ollama 适配器（含重试）
├── cache.rs         # SQLite 缓存（TTL + LRU 淘汰）+ 执行历史
├── danger.rs        # 危险检测 + 注入扫描（InjectionReason 枚举）
├── executor.rs      # 命令执行 + 用户确认
├── explain.rs       # 命令解释模式
├── fix.rs           # 命令纠错模式 + 自动修复重试循环
├── chat.rs          # 交互式对话模式（斜杠命令 + 历史持久化）
├── history.rs       # Shell 历史记录读取
├── shell_init.rs    # Shell 集成代码生成（bash/zsh/fish/PowerShell）
└── ui.rs            # 终端输出（Spinner、Diff、着色）
```

### 添加新 LLM 后端

1. 创建 `src/llm/your_backend.rs`
2. 实现 `LlmBackend` trait（`chat()` 和 `chat_with_history()`）
3. 使用 `super::should_retry()`、`super::backoff_delay()`、`super::MAX_RETRIES` 添加重试循环
4. 使用 `super::DEFAULT_TEMPERATURE` 和 `super::DEFAULT_MAX_TOKENS`
5. 在 `config.rs` 中添加配置结构体
6. 在 `src/llm/mod.rs` 的工厂函数 `create_backend()` 中注册
7. 在 `config.rs` 配置向导中添加设置流程
8. 编写测试

### 添加新语言

1. 在 `src/i18n.rs` 的 `Lang` 枚举中添加变体
2. 创建新的 `static` 翻译表（包含所有 `inject_*`、`chat_*` 和 `select_command` 字段）
3. 在 `t()` 函数中添加匹配分支
4. 更新 `config.rs` 中的语言选择器

### 添加新注入模式

1. 在 `src/danger.rs` 的 `InjectionReason` 枚举中添加变体
2. 在 `detect_injection()` 的模式列表中添加正则元组
3. 在 `src/i18n.rs` 的 `T` 结构体中添加 `inject_*` 字段
4. 为所有语言（中、英）添加翻译
5. 在 `InjectionReason::message()` 中添加匹配分支
6. 在 `danger.rs` 测试中添加测试用例
7. 更新 `i18n.rs` 中的 `all_langs_have_translations` 测试

## 行为准则

- 尊重他人，保持建设性
- 关注代码本身，而非个人
- 欢迎新人，帮助他们入门

## 许可证

参与贡献即表示你同意你的贡献将基于 MIT 许可证授权。
