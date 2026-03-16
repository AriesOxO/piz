pub mod claude;
pub mod gemini;
pub mod ollama;
pub mod openai;
pub mod prompt;

use crate::config::Config;
use anyhow::Result;
use async_trait::async_trait;

/// A message in a conversation
#[derive(Debug, Clone)]
pub struct Message {
    pub role: String, // "user" or "assistant"
    pub content: String,
}

#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn chat(&self, system: &str, user: &str) -> Result<String>;

    /// Multi-turn chat with message history
    async fn chat_with_history(&self, system: &str, messages: &[Message]) -> Result<String> {
        // Default: ignore history, just use the last user message
        if let Some(last) = messages.iter().rev().find(|m| m.role == "user") {
            self.chat(system, &last.content).await
        } else {
            anyhow::bail!("No user message in history")
        }
    }
}

/// A mock backend for testing that returns a preset response.
#[cfg(test)]
pub struct MockBackend {
    pub response: String,
}

#[cfg(test)]
#[async_trait]
impl LlmBackend for MockBackend {
    async fn chat(&self, _system: &str, _user: &str) -> Result<String> {
        Ok(self.response.clone())
    }
}

pub fn create_backend(
    config: &Config,
    backend_override: Option<&str>,
) -> Result<Box<dyn LlmBackend>> {
    let backend_name = backend_override.unwrap_or(&config.default_backend);
    match backend_name {
        "openai" => {
            let cfg = config
                .openai
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("OpenAI config not found in config.toml"))?;
            Ok(Box::new(openai::OpenAiBackend::new(cfg.clone())))
        }
        "claude" => {
            let cfg = config
                .claude
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Claude config not found in config.toml"))?;
            Ok(Box::new(claude::ClaudeBackend::new(cfg.clone())))
        }
        "gemini" => {
            let cfg = config
                .gemini
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Gemini config not found in config.toml"))?;
            Ok(Box::new(gemini::GeminiBackend::new(cfg.clone())))
        }
        "ollama" => {
            let cfg = config
                .ollama
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Ollama config not found in config.toml"))?;
            Ok(Box::new(ollama::OllamaBackend::new(cfg.clone())))
        }
        other => anyhow::bail!(
            "Unknown backend: {}. Supported: openai, claude, gemini, ollama",
            other
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;

    fn config_with_openai() -> Config {
        Config {
            default_backend: "openai".into(),
            cache_ttl_hours: 168,
            auto_confirm_safe: false,
            language: "zh".into(),
            openai: Some(OpenAiConfig {
                api_key: "sk-test".into(),
                model: "gpt-4o-mini".into(),
                base_url: None,
            }),
            claude: None,
            gemini: None,
            ollama: None,
        }
    }

    #[test]
    fn create_openai_backend() {
        let cfg = config_with_openai();
        let backend = create_backend(&cfg, None);
        assert!(backend.is_ok());
    }

    #[test]
    fn create_backend_with_override() {
        let mut cfg = config_with_openai();
        cfg.ollama = Some(OllamaConfig {
            host: "http://localhost:11434".into(),
            model: "llama3".into(),
        });
        let backend = create_backend(&cfg, Some("ollama"));
        assert!(backend.is_ok());
    }

    #[test]
    fn create_backend_unknown_errors() {
        let cfg = config_with_openai();
        let result = create_backend(&cfg, Some("unknown_backend"));
        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(err.contains("Unknown backend"));
    }

    #[test]
    fn create_backend_missing_config_errors() {
        let cfg = Config {
            default_backend: "claude".into(),
            cache_ttl_hours: 168,
            auto_confirm_safe: false,
            language: "zh".into(),
            openai: None,
            claude: None,
            gemini: None,
            ollama: None,
        };
        let result = create_backend(&cfg, None);
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap()
            .to_string()
            .contains("Claude config not found"));
    }

    #[tokio::test]
    async fn mock_backend_returns_response() {
        let mock = MockBackend {
            response: r#"{"command": "ls", "danger": "safe"}"#.into(),
        };
        let result = mock.chat("system", "user").await.unwrap();
        assert!(result.contains("ls"));
    }
}
