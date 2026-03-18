use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;

use super::{LlmBackend, Message};
use crate::config::OllamaConfig;

pub struct OllamaBackend {
    config: OllamaConfig,
    client: reqwest::Client,
}

impl OllamaBackend {
    pub fn new(config: OllamaConfig) -> Self {
        Self {
            config,
            client: super::build_http_client(),
        }
    }

    fn build_url(&self) -> String {
        format!("{}/api/chat", self.config.host.trim_end_matches('/'))
    }

    async fn send_request(&self, body: serde_json::Value) -> Result<String> {
        let url = self.build_url();
        let mut last_err = None;

        for attempt in 0..super::MAX_RETRIES {
            let resp = self
                .client
                .post(&url)
                .json(&body)
                .send()
                .await
                .context("Failed to send request to Ollama")?;

            let status = resp.status();
            let text = resp
                .text()
                .await
                .context("Failed to read Ollama response")?;

            if status.is_success() {
                let parsed: serde_json::Value =
                    serde_json::from_str(&text).context("Failed to parse Ollama response")?;
                return parsed["message"]["content"]
                    .as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| anyhow::anyhow!("Unexpected Ollama response format"));
            }

            match super::handle_error_response(status, &text, attempt, "Ollama") {
                Ok(msg) => {
                    last_err = Some(msg);
                    super::backoff_delay(attempt).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(super::bail_last_err(last_err, "Ollama request failed"))
    }
}

#[async_trait]
impl LlmBackend for OllamaBackend {
    async fn chat(&self, system: &str, user: &str) -> Result<String> {
        let body = json!({
            "model": self.config.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ],
            "stream": false,
            "format": "json",
            "options": {
                "temperature": super::DEFAULT_TEMPERATURE,
                "num_predict": super::DEFAULT_MAX_TOKENS
            }
        });
        self.send_request(body).await
    }

    async fn chat_with_history(&self, system: &str, messages: &[Message]) -> Result<String> {
        let mut msgs = vec![json!({"role": "system", "content": system})];
        for m in messages {
            msgs.push(json!({"role": m.role, "content": m.content}));
        }
        let body = json!({
            "model": self.config.model,
            "messages": msgs,
            "stream": false,
            "format": "json",
            "options": {
                "temperature": super::DEFAULT_TEMPERATURE,
                "num_predict": super::DEFAULT_MAX_TOKENS
            }
        });
        self.send_request(body).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(host: &str) -> OllamaConfig {
        OllamaConfig {
            host: host.into(),
            model: "llama3".into(),
        }
    }

    #[test]
    fn build_url_default() {
        let backend = OllamaBackend::new(make_config("http://localhost:11434"));
        assert_eq!(backend.build_url(), "http://localhost:11434/api/chat");
    }

    #[test]
    fn build_url_trailing_slash() {
        let backend = OllamaBackend::new(make_config("http://localhost:11434/"));
        assert_eq!(backend.build_url(), "http://localhost:11434/api/chat");
    }

    #[test]
    fn build_url_custom_host() {
        let backend = OllamaBackend::new(make_config("http://192.168.1.100:11434"));
        assert_eq!(backend.build_url(), "http://192.168.1.100:11434/api/chat");
    }

    #[test]
    fn response_content_extraction() {
        let response = r#"{"message":{"content":"echo hello"}}"#;
        let parsed: serde_json::Value = serde_json::from_str(response).unwrap();
        let content = parsed["message"]["content"].as_str();
        assert_eq!(content, Some("echo hello"));
    }

    #[test]
    fn response_missing_content_is_none() {
        let response = r#"{"message":{}}"#;
        let parsed: serde_json::Value = serde_json::from_str(response).unwrap();
        let content = parsed["message"]["content"].as_str();
        assert!(content.is_none());
    }

    #[test]
    fn config_model_preserved() {
        let backend = OllamaBackend::new(make_config("http://localhost:11434"));
        assert_eq!(backend.config.model, "llama3");
    }
}
