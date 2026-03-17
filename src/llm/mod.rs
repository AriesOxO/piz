pub mod claude;
pub mod gemini;
pub mod ollama;
pub mod openai;
pub mod prompt;

use crate::config::Config;
use anyhow::Result;
use async_trait::async_trait;
use std::time::Duration;

/// Default HTTP request timeout for LLM API calls
const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(120);

/// Default temperature for all LLM backends
pub(crate) const DEFAULT_TEMPERATURE: f64 = 0.1;

/// Default max tokens for all LLM backends
pub(crate) const DEFAULT_MAX_TOKENS: u32 = 2048;

/// Maximum number of retries for transient API errors
pub(crate) const MAX_RETRIES: u32 = 3;

/// Initial backoff delay in milliseconds
pub(crate) const INITIAL_BACKOFF_MS: u64 = 1000;

/// Check if an HTTP status code should trigger a retry
pub(crate) fn should_retry(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

/// Sleep with exponential backoff
pub(crate) async fn backoff_delay(attempt: u32) {
    let ms = INITIAL_BACKOFF_MS * 2u64.pow(attempt);
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

/// A message in a conversation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub role: String, // "user" or "assistant"
    pub content: String,
}

#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn chat(&self, system: &str, user: &str) -> Result<String>;

    /// Multi-turn chat with message history.
    /// All backends must implement this for chat mode to work correctly.
    async fn chat_with_history(&self, system: &str, messages: &[Message]) -> Result<String>;
}

/// Truncate a string to at most 500 characters for error preview
pub(crate) fn truncate_preview(text: &str) -> String {
    text.chars().take(500).collect()
}

/// Handle a non-success HTTP response: decide whether to retry or fail.
/// Returns `Ok(msg)` if should retry (msg is the error description for last_err),
/// or `Err(e)` if should bail immediately.
pub(crate) fn handle_error_response(
    status: reqwest::StatusCode,
    text: &str,
    attempt: u32,
    backend_name: &str,
) -> std::result::Result<String, anyhow::Error> {
    let preview = truncate_preview(text);
    if should_retry(status) && attempt + 1 < MAX_RETRIES {
        Ok(format!(
            "{} API error ({}): {}",
            backend_name, status, preview
        ))
    } else {
        Err(anyhow::anyhow!(
            "{} API error ({}): {}",
            backend_name,
            status,
            preview
        ))
    }
}

/// Build a final bail error from the last retry error message
pub(crate) fn bail_last_err(last_err: Option<String>, default_msg: &str) -> anyhow::Error {
    anyhow::anyhow!("{}", last_err.unwrap_or_else(|| default_msg.into()))
}

/// Create a shared reqwest::Client with timeout configured
pub(crate) fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(DEFAULT_HTTP_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
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

    async fn chat_with_history(&self, _system: &str, _messages: &[Message]) -> Result<String> {
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
            chat_history_size: 20,
            cache_max_entries: 1000,
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
            chat_history_size: 20,
            cache_max_entries: 1000,
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

    #[test]
    fn message_serialization_roundtrip() {
        let msgs = vec![
            Message {
                role: "user".into(),
                content: "hello".into(),
            },
            Message {
                role: "assistant".into(),
                content: "hi".into(),
            },
        ];
        let json = serde_json::to_string(&msgs).unwrap();
        let parsed: Vec<Message> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].role, "user");
        assert_eq!(parsed[1].content, "hi");
    }

    #[test]
    fn should_retry_on_429() {
        assert!(should_retry(reqwest::StatusCode::TOO_MANY_REQUESTS));
    }

    #[test]
    fn should_retry_on_500() {
        assert!(should_retry(reqwest::StatusCode::INTERNAL_SERVER_ERROR));
    }

    #[test]
    fn should_retry_on_503() {
        assert!(should_retry(reqwest::StatusCode::SERVICE_UNAVAILABLE));
    }

    #[test]
    fn should_not_retry_on_400() {
        assert!(!should_retry(reqwest::StatusCode::BAD_REQUEST));
    }

    #[test]
    fn should_not_retry_on_401() {
        assert!(!should_retry(reqwest::StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn should_not_retry_on_200() {
        assert!(!should_retry(reqwest::StatusCode::OK));
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
