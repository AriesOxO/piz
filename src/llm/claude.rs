use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;

use super::{LlmBackend, Message};
use crate::config::ClaudeConfig;

pub struct ClaudeBackend {
    config: ClaudeConfig,
    client: reqwest::Client,
}

impl ClaudeBackend {
    pub fn new(config: ClaudeConfig) -> Self {
        Self {
            config,
            client: super::build_http_client(),
        }
    }

    fn build_url(&self) -> String {
        let base = self
            .config
            .base_url
            .as_deref()
            .unwrap_or("https://api.anthropic.com");
        format!("{}/v1/messages", base.trim_end_matches('/'))
    }

    async fn send_request(&self, body: serde_json::Value) -> Result<String> {
        let url = self.build_url();
        let mut last_err = None;

        for attempt in 0..super::MAX_RETRIES {
            let resp = self
                .client
                .post(&url)
                .header("x-api-key", &self.config.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .context("Failed to send request to Claude")?;

            let status = resp.status();
            let text = resp
                .text()
                .await
                .context("Failed to read Claude response")?;

            if status.is_success() {
                let parsed: serde_json::Value =
                    serde_json::from_str(&text).context("Failed to parse Claude response")?;
                return parsed["content"][0]["text"]
                    .as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| anyhow::anyhow!("Unexpected Claude response format"));
            }

            match super::handle_error_response(status, &text, attempt, "Claude") {
                Ok(msg) => {
                    last_err = Some(msg);
                    super::backoff_delay(attempt).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(super::bail_last_err(last_err, "Claude request failed"))
    }
}

#[async_trait]
impl LlmBackend for ClaudeBackend {
    async fn chat(&self, system: &str, user: &str) -> Result<String> {
        let body = json!({
            "model": self.config.model,
            "max_tokens": super::DEFAULT_MAX_TOKENS,
            "temperature": super::DEFAULT_TEMPERATURE,
            "system": system,
            "messages": [
                {"role": "user", "content": user}
            ]
        });
        self.send_request(body).await
    }

    async fn chat_with_history(&self, system: &str, messages: &[Message]) -> Result<String> {
        let msgs: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| json!({"role": m.role, "content": m.content}))
            .collect();
        let body = json!({
            "model": self.config.model,
            "max_tokens": super::DEFAULT_MAX_TOKENS,
            "temperature": super::DEFAULT_TEMPERATURE,
            "system": system,
            "messages": msgs
        });
        self.send_request(body).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(base_url: Option<&str>) -> ClaudeConfig {
        ClaudeConfig {
            api_key: "test-key".into(),
            model: "claude-sonnet-4-20250514".into(),
            base_url: base_url.map(|s| s.into()),
        }
    }

    #[test]
    fn build_url_default() {
        let backend = ClaudeBackend::new(make_config(None));
        assert_eq!(backend.build_url(), "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn build_url_custom_base() {
        let backend = ClaudeBackend::new(make_config(Some("https://my-proxy.com")));
        assert_eq!(backend.build_url(), "https://my-proxy.com/v1/messages");
    }

    #[test]
    fn build_url_trailing_slash_stripped() {
        let backend = ClaudeBackend::new(make_config(Some("https://my-proxy.com/")));
        assert_eq!(backend.build_url(), "https://my-proxy.com/v1/messages");
    }

    #[test]
    fn build_url_preserves_path_prefix() {
        let backend = ClaudeBackend::new(make_config(Some("https://my-proxy.com/prefix")));
        assert_eq!(
            backend.build_url(),
            "https://my-proxy.com/prefix/v1/messages"
        );
    }

    #[test]
    fn config_model_preserved() {
        let backend = ClaudeBackend::new(make_config(None));
        assert_eq!(backend.config.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn response_text_extraction() {
        let response = r#"{"content":[{"text":"hello world"}]}"#;
        let parsed: serde_json::Value = serde_json::from_str(response).unwrap();
        let text = parsed["content"][0]["text"].as_str();
        assert_eq!(text, Some("hello world"));
    }

    #[test]
    fn response_unexpected_format() {
        let response = r#"{"content":[]}"#;
        let parsed: serde_json::Value = serde_json::from_str(response).unwrap();
        let text = parsed["content"][0]["text"].as_str();
        assert!(text.is_none());
    }
}
