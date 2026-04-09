use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::i18n::{self, Lang};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_backend")]
    pub default_backend: String,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_hours: u64,
    #[serde(default)]
    pub auto_confirm_safe: bool,
    #[serde(default)]
    pub show_explanation: bool,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_chat_history_size")]
    pub chat_history_size: usize,
    #[serde(default = "default_cache_max_entries")]
    pub cache_max_entries: usize,
    pub openai: Option<OpenAiConfig>,
    pub claude: Option<ClaudeConfig>,
    pub gemini: Option<GeminiConfig>,
    pub ollama: Option<OllamaConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OpenAiConfig {
    pub api_key: String,
    #[serde(default = "default_openai_model")]
    pub model: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ClaudeConfig {
    pub api_key: String,
    #[serde(default = "default_claude_model")]
    pub model: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GeminiConfig {
    pub api_key: String,
    #[serde(default = "default_gemini_model")]
    pub model: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OllamaConfig {
    #[serde(default = "default_ollama_host")]
    pub host: String,
    #[serde(default = "default_ollama_model")]
    pub model: String,
}

fn default_backend() -> String {
    "openai".into()
}
fn default_cache_ttl() -> u64 {
    48
}
fn default_language() -> String {
    "zh".into()
}
fn default_openai_model() -> String {
    "gpt-4o-mini".into()
}
fn default_claude_model() -> String {
    "claude-sonnet-4-20250514".into()
}
fn default_chat_history_size() -> usize {
    20
}
fn default_cache_max_entries() -> usize {
    1000
}
fn default_gemini_model() -> String {
    "gemini-2.5-flash".into()
}
fn default_ollama_host() -> String {
    "http://localhost:11434".into()
}
fn default_ollama_model() -> String {
    "llama3".into()
}

pub fn piz_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .ok()
        .or_else(dirs::home_dir)
        .context("Cannot determine home directory")?;
    Ok(home.join(".piz"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(piz_dir()?.join("config.toml"))
}

pub fn load_config() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        println!("No configuration found. Let's set up piz for the first time.\n");
        init_config()?;
        // If init was aborted or config still missing, bail
        if !path.exists() {
            anyhow::bail!("Config file not created. Run `piz config --init` to try again.");
        }
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let config: Config =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(config)
}

pub fn init_config() -> Result<()> {
    let dir = piz_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = config_path()?;

    // Step 0: choose language first (always show trilingual)
    let lang_items = &["中文", "English"];
    let lang_idx = dialoguer::Select::new()
        .with_prompt("选择语言 / Select language")
        .items(lang_items)
        .default(0)
        .interact()?;
    let lang = match lang_idx {
        1 => Lang::En,
        _ => Lang::Zh,
    };
    let tr = i18n::t(lang);

    if path.exists() {
        let overwrite = dialoguer::Confirm::new()
            .with_prompt(format!("{} ({})", tr.config_overwrite, path.display()))
            .default(false)
            .interact()?;
        if !overwrite {
            println!("{}", tr.cancelled);
            return Ok(());
        }
    }

    println!();
    println!("  {}", tr.wizard_title);
    println!();

    // Step 1: choose backend
    let backends = &[
        "openai (DeepSeek, SiliconFlow, OpenRouter, ...)",
        "claude",
        "gemini (Google)",
        "ollama (local)",
    ];
    let backend_idx = dialoguer::Select::new()
        .with_prompt(tr.select_backend)
        .items(backends)
        .default(0)
        .interact()?;
    let backend_name = match backend_idx {
        0 => "openai",
        1 => "claude",
        2 => "gemini",
        3 => "ollama",
        _ => "openai",
    };

    // Step 2: collect backend-specific config
    let mut openai_section = String::new();
    let mut claude_section = String::new();
    let mut gemini_section = String::new();
    let mut ollama_section = String::new();

    match backend_name {
        "openai" => openai_section = collect_openai_config(tr)?,
        "claude" => claude_section = collect_claude_config(tr)?,
        "gemini" => gemini_section = collect_gemini_config(tr)?,
        "ollama" => ollama_section = collect_ollama_config(tr)?,
        _ => {}
    }

    // Step 3: auto_confirm
    let auto_confirm = dialoguer::Confirm::new()
        .with_prompt(tr.auto_confirm_prompt)
        .default(true)
        .interact()?;

    // Step 4: additional backends
    let extra = dialoguer::Confirm::new()
        .with_prompt(tr.extra_backends)
        .default(false)
        .interact()?;

    if extra {
        if backend_name != "openai"
            && openai_section.is_empty()
            && dialoguer::Confirm::new()
                .with_prompt(tr.add_openai)
                .default(false)
                .interact()?
        {
            openai_section = collect_openai_config(tr)?;
        }
        if backend_name != "claude"
            && claude_section.is_empty()
            && dialoguer::Confirm::new()
                .with_prompt(tr.add_claude)
                .default(false)
                .interact()?
        {
            claude_section = collect_claude_config(tr)?;
        }
        if backend_name != "gemini"
            && gemini_section.is_empty()
            && dialoguer::Confirm::new()
                .with_prompt(tr.add_gemini)
                .default(false)
                .interact()?
        {
            gemini_section = collect_gemini_config(tr)?;
        }
        if backend_name != "ollama"
            && ollama_section.is_empty()
            && dialoguer::Confirm::new()
                .with_prompt(tr.add_ollama)
                .default(false)
                .interact()?
        {
            ollama_section = collect_ollama_config(tr)?;
        }
    }

    // Assemble config
    let mut content = format!(
        "default_backend = \"{backend}\"\n\
         cache_ttl_hours = 48\n\
         auto_confirm_safe = {auto_confirm}\n\
         language = \"{lang}\"\n",
        backend = backend_name,
        auto_confirm = auto_confirm,
        lang = lang.code(),
    );

    if !openai_section.is_empty() {
        content.push_str(&format!("\n{}", openai_section));
    }
    if !claude_section.is_empty() {
        content.push_str(&format!("\n{}", claude_section));
    }
    if !gemini_section.is_empty() {
        content.push_str(&format!("\n{}", gemini_section));
    }
    if !ollama_section.is_empty() {
        content.push_str(&format!("\n{}", ollama_section));
    }

    std::fs::write(&path, &content)?;

    println!();
    println!("  {} {}", tr.config_saved, path.display());
    println!();

    match parse_config(&content) {
        Ok(_) => println!("  {}", tr.config_validated),
        Err(e) => println!("  ⚠ {}", e),
    }

    println!();
    println!("  {} {}", tr.config_edit_hint, path.display());
    println!("  {}", tr.config_rerun_hint);
    println!();
    Ok(())
}

fn collect_openai_config(tr: &i18n::T) -> Result<String> {
    println!();
    println!("  -- OpenAI-compatible --");

    let presets = &[
        "OpenAI (api.openai.com)",
        "DeepSeek (api.deepseek.com)",
        "SiliconFlow (api.siliconflow.cn)",
        "OpenRouter (openrouter.ai)",
        "Moonshot / Kimi (api.moonshot.cn)",
        "Zhipu / GLM (open.bigmodel.cn)",
        "Qianfan / Baidu (qianfan.baidubce.com)",
        "DashScope / Alibaba (dashscope.aliyuncs.com)",
        "Mistral (api.mistral.ai)",
        "Together (api.together.xyz)",
        "Minimax (api.minimax.io)",
        "BytePlus / Volcengine (byteplus)",
        "Custom URL",
    ];
    let preset_idx = dialoguer::Select::new()
        .with_prompt(tr.select_provider)
        .items(presets)
        .default(0)
        .interact()?;

    let (default_url, default_model) = match preset_idx {
        0 => ("https://api.openai.com", "gpt-4o-mini"),
        1 => ("https://api.deepseek.com", "deepseek-chat"),
        2 => ("https://api.siliconflow.cn", "Qwen/Qwen3-8B"),
        3 => ("https://openrouter.ai/api/v1", "auto"),
        4 => ("https://api.moonshot.cn", "moonshot-v1-8k"),
        5 => ("https://open.bigmodel.cn/api/paas/v4", "glm-4-flash"),
        6 => ("https://qianfan.baidubce.com/v2", "deepseek-v3"),
        7 => (
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "qwen-plus",
        ),
        8 => ("https://api.mistral.ai/v1", "mistral-small-latest"),
        9 => (
            "https://api.together.xyz/v1",
            "meta-llama/Meta-Llama-3-8B-Instruct",
        ),
        10 => ("https://api.minimax.io/v1", "MiniMax-M1"),
        11 => (
            "https://api.byteplus.volcengineapi.com/v1",
            "doubao-1.5-pro-32k",
        ),
        _ => ("", ""),
    };

    let base_url: String = dialoguer::Input::new()
        .with_prompt(format!("  {}", tr.base_url_prompt))
        .with_initial_text(default_url)
        .interact_text()?;

    let api_key: String = dialoguer::Input::new()
        .with_prompt(format!("  {}", tr.api_key_prompt))
        .interact_text()?;

    if api_key.is_empty() {
        anyhow::bail!("API key cannot be empty");
    }

    let model: String = dialoguer::Input::new()
        .with_prompt(format!("  {}", tr.model_prompt))
        .with_initial_text(default_model)
        .interact_text()?;

    let mut section = format!(
        "[openai]\napi_key = {}\nmodel = {}\n",
        toml_escape(&api_key),
        toml_escape(&model)
    );
    if !base_url.is_empty() && base_url != "https://api.openai.com" {
        section.push_str(&format!("base_url = {}\n", toml_escape(&base_url)));
    }

    Ok(section)
}

fn collect_claude_config(tr: &i18n::T) -> Result<String> {
    println!();
    println!("  -- Claude --");

    let api_key: String = dialoguer::Input::new()
        .with_prompt(format!("  {}", tr.api_key_prompt))
        .interact_text()?;

    if api_key.is_empty() {
        anyhow::bail!("API key cannot be empty");
    }

    let model: String = dialoguer::Input::new()
        .with_prompt(format!("  {}", tr.model_prompt))
        .with_initial_text("claude-sonnet-4-20250514")
        .interact_text()?;

    let use_custom_url = dialoguer::Confirm::new()
        .with_prompt(tr.custom_url_prompt)
        .default(false)
        .interact()?;

    let mut section = format!(
        "[claude]\napi_key = {}\nmodel = {}\n",
        toml_escape(&api_key),
        toml_escape(&model)
    );

    if use_custom_url {
        let base_url: String = dialoguer::Input::new()
            .with_prompt(format!("  {}", tr.base_url_prompt))
            .with_initial_text("https://api.anthropic.com")
            .interact_text()?;
        section.push_str(&format!("base_url = {}\n", toml_escape(&base_url)));
    }

    Ok(section)
}

fn collect_gemini_config(tr: &i18n::T) -> Result<String> {
    println!();
    println!("  -- Google Gemini --");

    let api_key: String = dialoguer::Input::new()
        .with_prompt(format!("  {}", tr.api_key_prompt))
        .interact_text()?;

    if api_key.is_empty() {
        anyhow::bail!("API key cannot be empty");
    }

    let model: String = dialoguer::Input::new()
        .with_prompt(format!("  {}", tr.model_prompt))
        .with_initial_text("gemini-2.5-flash")
        .interact_text()?;

    Ok(format!(
        "[gemini]\napi_key = {}\nmodel = {}\n",
        toml_escape(&api_key),
        toml_escape(&model)
    ))
}

fn collect_ollama_config(tr: &i18n::T) -> Result<String> {
    println!();
    println!("  -- Ollama (local) --");

    let host: String = dialoguer::Input::new()
        .with_prompt(format!("  {}", tr.ollama_host))
        .with_initial_text("http://localhost:11434")
        .interact_text()?;

    let model: String = dialoguer::Input::new()
        .with_prompt(format!("  {}", tr.model_prompt))
        .with_initial_text("llama3")
        .interact_text()?;

    Ok(format!(
        "[ollama]\nhost = {}\nmodel = {}\n",
        toml_escape(&host),
        toml_escape(&model)
    ))
}

/// Return a stable identifier for the active backend+model combination.
/// Used as part of the cache key so switching models auto-invalidates stale entries.
pub fn active_model_id(config: &Config, backend_override: Option<&str>) -> String {
    let backend = backend_override.unwrap_or(&config.default_backend);
    let model = match backend {
        "openai" => config.openai.as_ref().map(|c| c.model.as_str()),
        "claude" => config.claude.as_ref().map(|c| c.model.as_str()),
        "gemini" => config.gemini.as_ref().map(|c| c.model.as_str()),
        "ollama" => config.ollama.as_ref().map(|c| c.model.as_str()),
        _ => None,
    };
    format!("{}:{}", backend, model.unwrap_or("default"))
}

/// Escape a string value for TOML output (handles quotes, backslashes, etc.)
fn toml_escape(s: &str) -> String {
    let value = toml::Value::String(s.to_string());
    value.to_string()
}

/// Mask an API key for display: show first 4 and last 4 chars if long enough
pub fn mask_api_key(key: &str) -> String {
    if key.len() > 8 {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    } else {
        "*".repeat(key.len())
    }
}

/// Mask all api_key values in a TOML config string
pub fn mask_config_keys(content: &str) -> String {
    content
        .lines()
        .map(|line| {
            if let Some(idx) = line.find("api_key") {
                if let Some(eq_idx) = line[idx..].find('=') {
                    let before = &line[..idx + eq_idx + 1];
                    let value_part = line[idx + eq_idx + 1..].trim();
                    if value_part.starts_with('"')
                        && value_part.ends_with('"')
                        && value_part.len() > 2
                    {
                        let key = &value_part[1..value_part.len() - 1];
                        return format!("{} \"{}\"", before, mask_api_key(key));
                    }
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse a TOML string into Config (useful for testing)
pub fn parse_config(content: &str) -> Result<Config> {
    let config: Config = toml::from_str(content).context("Failed to parse config TOML")?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_config() {
        let toml = r#"
default_backend = "claude"
cache_ttl_hours = 24
auto_confirm_safe = true

[openai]
api_key = "sk-test"
model = "gpt-4"
base_url = "https://custom.api.com"

[claude]
api_key = "sk-ant-test"
model = "claude-sonnet-4-20250514"

[ollama]
host = "http://localhost:11434"
model = "llama3"
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.default_backend, "claude");
        assert_eq!(cfg.cache_ttl_hours, 24);
        assert!(cfg.auto_confirm_safe);

        let openai = cfg.openai.unwrap();
        assert_eq!(openai.api_key, "sk-test");
        assert_eq!(openai.model, "gpt-4");
        assert_eq!(openai.base_url.unwrap(), "https://custom.api.com");

        let claude = cfg.claude.unwrap();
        assert_eq!(claude.api_key, "sk-ant-test");

        let ollama = cfg.ollama.unwrap();
        assert_eq!(ollama.host, "http://localhost:11434");
        assert_eq!(ollama.model, "llama3");
    }

    #[test]
    fn parse_minimal_config_uses_defaults() {
        let toml = r#"
[openai]
api_key = "sk-test"
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.default_backend, "openai");
        assert_eq!(cfg.cache_ttl_hours, 48);
        assert!(!cfg.auto_confirm_safe);
        assert_eq!(cfg.openai.unwrap().model, "gpt-4o-mini");
        assert!(cfg.claude.is_none());
        assert!(cfg.ollama.is_none());
    }

    #[test]
    fn parse_empty_config_ok() {
        let cfg = parse_config("").unwrap();
        assert_eq!(cfg.default_backend, "openai");
        assert!(cfg.openai.is_none());
    }

    #[test]
    fn parse_invalid_toml_errors() {
        let result = parse_config("this is not [valid toml");
        assert!(result.is_err());
    }

    #[test]
    fn mask_api_key_long() {
        assert_eq!(mask_api_key("sk-test-key-123456"), "sk-t...3456");
    }

    #[test]
    fn mask_api_key_short() {
        assert_eq!(mask_api_key("short"), "*****");
    }

    #[test]
    fn mask_api_key_exact_8() {
        assert_eq!(mask_api_key("12345678"), "********");
    }

    #[test]
    fn mask_config_keys_masks_values() {
        let content = "[openai]\napi_key = \"sk-test-key-123456\"\nmodel = \"gpt-4\"";
        let masked = mask_config_keys(content);
        assert!(masked.contains("sk-t...3456"));
        assert!(!masked.contains("sk-test-key-123456"));
        assert!(masked.contains("gpt-4"));
    }

    #[test]
    fn show_explanation_default_false() {
        let cfg = parse_config("").unwrap();
        assert!(!cfg.show_explanation);
    }

    #[test]
    fn show_explanation_true() {
        let cfg = parse_config("show_explanation = true").unwrap();
        assert!(cfg.show_explanation);
    }

    #[test]
    fn show_explanation_missing_defaults_false() {
        let toml = r#"
default_backend = "openai"
auto_confirm_safe = true
[openai]
api_key = "sk-test"
"#;
        let cfg = parse_config(toml).unwrap();
        assert!(!cfg.show_explanation);
    }

    #[test]
    fn piz_dir_under_home() {
        let dir = piz_dir().unwrap();
        assert!(dir.ends_with(".piz"));
    }

    #[test]
    fn config_path_is_toml() {
        let path = config_path().unwrap();
        assert_eq!(path.extension().unwrap(), "toml");
        assert!(path.ends_with("config.toml"));
    }

    // ── Backend-specific config parsing ──

    #[test]
    fn parse_claude_config_standalone() {
        let toml = r#"
default_backend = "claude"
[claude]
api_key = "sk-ant-test"
model = "claude-sonnet-4-20250514"
"#;
        let cfg = parse_config(toml).unwrap();
        assert!(cfg.claude.is_some());
        let claude = cfg.claude.unwrap();
        assert_eq!(claude.api_key, "sk-ant-test");
        assert_eq!(claude.model, "claude-sonnet-4-20250514");
        assert!(claude.base_url.is_none());
    }

    #[test]
    fn parse_claude_config_with_base_url() {
        let toml = r#"
[claude]
api_key = "sk-test"
base_url = "https://my-proxy.com"
"#;
        let cfg = parse_config(toml).unwrap();
        let claude = cfg.claude.unwrap();
        assert_eq!(claude.base_url.as_deref(), Some("https://my-proxy.com"));
    }

    #[test]
    fn parse_gemini_config_standalone() {
        let toml = r#"
default_backend = "gemini"
[gemini]
api_key = "AIza-test"
model = "gemini-2.5-flash"
"#;
        let cfg = parse_config(toml).unwrap();
        assert!(cfg.gemini.is_some());
        let gemini = cfg.gemini.unwrap();
        assert_eq!(gemini.api_key, "AIza-test");
        assert_eq!(gemini.model, "gemini-2.5-flash");
    }

    #[test]
    fn parse_ollama_config_defaults() {
        let toml = r#"
[ollama]
"#;
        let cfg = parse_config(toml).unwrap();
        let ollama = cfg.ollama.unwrap();
        assert_eq!(ollama.host, "http://localhost:11434");
        assert_eq!(ollama.model, "llama3");
    }

    // ── Default values ──

    #[test]
    fn cache_ttl_default() {
        let cfg = parse_config("").unwrap();
        assert_eq!(cfg.cache_ttl_hours, 48);
    }

    #[test]
    fn cache_max_entries_default() {
        let cfg = parse_config("").unwrap();
        assert_eq!(cfg.cache_max_entries, 1000);
    }

    #[test]
    fn chat_history_size_default() {
        let cfg = parse_config("").unwrap();
        assert_eq!(cfg.chat_history_size, 20);
    }

    #[test]
    fn auto_confirm_safe_default() {
        let cfg = parse_config("").unwrap();
        assert!(!cfg.auto_confirm_safe);
    }

    #[test]
    fn language_default_zh() {
        let cfg = parse_config("").unwrap();
        assert_eq!(cfg.language, "zh");
    }

    #[test]
    fn language_custom() {
        let cfg = parse_config("language = \"en\"").unwrap();
        assert_eq!(cfg.language, "en");
    }

    #[test]
    fn default_backend_openai() {
        let cfg = parse_config("").unwrap();
        assert_eq!(cfg.default_backend, "openai");
    }

    // ── mask_api_key edge cases ──

    #[test]
    fn mask_api_key_empty() {
        // empty string → 0 * chars
        assert_eq!(mask_api_key(""), "");
    }

    #[test]
    fn mask_api_key_single_char() {
        // <=8 chars → repeat '*' for each char
        assert_eq!(mask_api_key("a"), "*");
    }
}
