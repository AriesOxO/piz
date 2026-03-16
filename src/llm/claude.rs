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
        let resp = self
            .client
            .post(self.build_url())
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

        if !status.is_success() {
            let preview: String = text.chars().take(500).collect();
            anyhow::bail!("Claude API error ({}): {}", status, preview);
        }

        let parsed: serde_json::Value =
            serde_json::from_str(&text).context("Failed to parse Claude response")?;

        parsed["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Unexpected Claude response format"))
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
