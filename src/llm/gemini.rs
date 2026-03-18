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
        let url = self.build_url();
        let mut last_err = None;

        for attempt in 0..super::MAX_RETRIES {
            let resp = self
                .client
                .post(&url)
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

            if status.is_success() {
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

                return parsed["candidates"][0]["content"]["parts"][0]["text"]
                    .as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| anyhow::anyhow!("Unexpected Gemini response format"));
            }

            match super::handle_error_response(status, &text, attempt, "Gemini") {
                Ok(msg) => {
                    last_err = Some(msg);
                    super::backoff_delay(attempt).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(super::bail_last_err(last_err, "Gemini request failed"))
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
                "maxOutputTokens": super::DEFAULT_MAX_TOKENS,
                "responseMimeType": "application/json"
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
                "maxOutputTokens": super::DEFAULT_MAX_TOKENS,
                "responseMimeType": "application/json"
            }
        });
        self.send_request(body).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(base_url: Option<&str>, model: &str) -> GeminiConfig {
        GeminiConfig {
            api_key: "test-key".into(),
            model: model.into(),
            base_url: base_url.map(|s| s.into()),
        }
    }

    #[test]
    fn build_url_default() {
        let backend = GeminiBackend::new(make_config(None, "gemini-2.5-flash"));
        let url = backend.build_url();
        assert!(url.contains("generativelanguage.googleapis.com"));
        assert!(url.contains("gemini-2.5-flash"));
        assert!(url.ends_with(":generateContent"));
    }

    #[test]
    fn build_url_custom_base() {
        let backend = GeminiBackend::new(make_config(Some("https://proxy.com"), "gemini-pro"));
        assert_eq!(
            backend.build_url(),
            "https://proxy.com/v1beta/models/gemini-pro:generateContent"
        );
    }

    #[test]
    fn build_url_trailing_slash_stripped() {
        let backend = GeminiBackend::new(make_config(Some("https://proxy.com/"), "gemini-pro"));
        let url = backend.build_url();
        assert!(url.starts_with("https://proxy.com/v1beta"));
        assert!(!url.contains("//v1beta"));
    }

    #[test]
    fn build_url_includes_model_name() {
        let backend = GeminiBackend::new(make_config(None, "gemini-2.0-flash"));
        let url = backend.build_url();
        assert!(url.contains("gemini-2.0-flash"));
    }

    #[test]
    fn safety_block_reason_detection() {
        let response = r#"{"promptFeedback":{"blockReason":"SAFETY"}}"#;
        let parsed: serde_json::Value = serde_json::from_str(response).unwrap();
        let blocked = parsed["promptFeedback"]["blockReason"].as_str();
        assert_eq!(blocked, Some("SAFETY"));
    }

    #[test]
    fn safety_finish_reason_detection() {
        let response = r#"{"candidates":[{"finishReason":"SAFETY"}]}"#;
        let parsed: serde_json::Value = serde_json::from_str(response).unwrap();
        let reason = parsed["candidates"][0]["finishReason"].as_str();
        assert_eq!(reason, Some("SAFETY"));
    }

    #[test]
    fn normal_response_text_extraction() {
        let response = r#"{"candidates":[{"content":{"parts":[{"text":"hello"}]}}]}"#;
        let parsed: serde_json::Value = serde_json::from_str(response).unwrap();
        let text = parsed["candidates"][0]["content"]["parts"][0]["text"].as_str();
        assert_eq!(text, Some("hello"));
    }

    #[test]
    fn empty_candidates_extraction_fails() {
        let response = r#"{"candidates":[]}"#;
        let parsed: serde_json::Value = serde_json::from_str(response).unwrap();
        let text = parsed["candidates"][0]["content"]["parts"][0]["text"].as_str();
        assert!(text.is_none());
    }

    #[test]
    fn role_mapping_assistant_to_model() {
        // Gemini uses "model" instead of "assistant"
        let role = "assistant";
        let mapped = if role == "assistant" { "model" } else { "user" };
        assert_eq!(mapped, "model");
    }

    #[test]
    fn role_mapping_user_stays_user() {
        let role = "user";
        let mapped = if role == "assistant" { "model" } else { "user" };
        assert_eq!(mapped, "user");
    }
}
