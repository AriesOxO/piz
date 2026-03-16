use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;

use super::{LlmBackend, Message};
use crate::config::OpenAiConfig;

pub struct OpenAiBackend {
    config: OpenAiConfig,
    client: reqwest::Client,
}

impl OpenAiBackend {
    pub fn new(config: OpenAiConfig) -> Self {
        Self {
            config,
            client: super::build_http_client(),
        }
    }

    fn build_url(&self) -> String {
        let base_url = self
            .config
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com");
        format!("{}/v1/chat/completions", base_url.trim_end_matches('/'))
    }

    async fn send_request(&self, body: serde_json::Value) -> Result<String> {
        let url = self.build_url();
        let mut last_err = None;

        for attempt in 0..super::MAX_RETRIES {
            let resp = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .context("Failed to send request to OpenAI")?;

            let status = resp.status();
            let text = resp
                .text()
                .await
                .context("Failed to read OpenAI response")?;

            if status.is_success() {
                let parsed: serde_json::Value =
                    serde_json::from_str(&text).context("Failed to parse OpenAI response")?;
                return parsed["choices"][0]["message"]["content"]
                    .as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| anyhow::anyhow!("Unexpected OpenAI response format"));
            }

            if super::should_retry(status) && attempt + 1 < super::MAX_RETRIES {
                super::backoff_delay(attempt).await;
                last_err = Some(format!("OpenAI API error ({}): {}", status, text.chars().take(500).collect::<String>()));
                continue;
            }

            let preview: String = text.chars().take(500).collect();
            anyhow::bail!("OpenAI API error ({}): {}", status, preview);
        }

        anyhow::bail!("{}", last_err.unwrap_or_else(|| "OpenAI request failed".into()))
    }
}

#[async_trait]
impl LlmBackend for OpenAiBackend {
    async fn chat(&self, system: &str, user: &str) -> Result<String> {
        let body = json!({
            "model": self.config.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ],
            "temperature": super::DEFAULT_TEMPERATURE,
            "max_tokens": super::DEFAULT_MAX_TOKENS
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
            "temperature": super::DEFAULT_TEMPERATURE,
            "max_tokens": super::DEFAULT_MAX_TOKENS
        });
        self.send_request(body).await
    }
}
