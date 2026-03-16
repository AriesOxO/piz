use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;

use super::{LlmBackend, Message};
use crate::config::GeminiConfig;

pub struct GeminiBackend {
    config: GeminiConfig,
    client: reqwest::Client,
}

impl GeminiBackend {
    pub fn new(config: GeminiConfig) -> Self {
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
            .unwrap_or("https://generativelanguage.googleapis.com");
        format!(
            "{}/v1beta/models/{}:generateContent",
            base.trim_end_matches('/'),
            self.config.model,
        )
    }

    async fn send_request(&self, body: serde_json::Value) -> Result<String> {
        let resp = self
            .client
            .post(self.build_url())
            .header("Content-Type", "application/json")
            .header("x-goog-api-key", &self.config.api_key)
            .json(&body)
            .send()
            .await
            .context("Failed to send request to Gemini")?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .context("Failed to read Gemini response")?;

        if !status.is_success() {
            let preview: String = text.chars().take(500).collect();
            anyhow::bail!("Gemini API error ({}): {}", status, preview);
        }

        let parsed: serde_json::Value =
            serde_json::from_str(&text).context("Failed to parse Gemini response")?;

        // Check for safety block
        if let Some(reason) = parsed["promptFeedback"]["blockReason"].as_str() {
            anyhow::bail!("Gemini blocked the request: {}", reason);
        }
        if let Some(reason) = parsed["candidates"][0]["finishReason"].as_str() {
            if reason == "SAFETY" {
                anyhow::bail!("Gemini response blocked due to safety filters");
            }
        }

        parsed["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Unexpected Gemini response format"))
    }
}

#[async_trait]
impl LlmBackend for GeminiBackend {
    async fn chat(&self, system: &str, user: &str) -> Result<String> {
        let body = json!({
            "system_instruction": {
                "parts": [{"text": system}]
            },
            "contents": [
                {
                    "role": "user",
                    "parts": [{"text": user}]
                }
            ],
            "generationConfig": {
                "temperature": super::DEFAULT_TEMPERATURE,
                "maxOutputTokens": super::DEFAULT_MAX_TOKENS
            }
        });
        self.send_request(body).await
    }

    async fn chat_with_history(&self, system: &str, messages: &[Message]) -> Result<String> {
        let contents: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                let role = if m.role == "assistant" {
                    "model"
                } else {
                    "user"
                };
                json!({
                    "role": role,
                    "parts": [{"text": m.content}]
                })
            })
            .collect();
        let body = json!({
            "system_instruction": {
                "parts": [{"text": system}]
            },
            "contents": contents,
            "generationConfig": {
                "temperature": super::DEFAULT_TEMPERATURE,
                "maxOutputTokens": super::DEFAULT_MAX_TOKENS
            }
        });
        self.send_request(body).await
    }
}
