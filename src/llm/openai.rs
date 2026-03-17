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
        let normalized = base_url.trim_end_matches('/');
        let lower = normalized.to_ascii_lowercase();

        if lower.ends_with("/chat/completions") {
            return normalized.to_string();
        }

        if lower.ends_with("/v1") {
            return format!("{}/chat/completions", normalized);
        }

        format!("{}/v1/chat/completions", normalized)
    }

    fn should_omit_temperature_by_default(&self) -> bool {
        let model = self.config.model.trim().to_ascii_lowercase();
        model.contains("kimi-k2.5")
    }

    fn build_body(
        &self,
        messages: Vec<serde_json::Value>,
        include_temperature: bool,
    ) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert("model".to_string(), json!(self.config.model));
        map.insert("messages".to_string(), json!(messages));
        map.insert("max_tokens".to_string(), json!(super::DEFAULT_MAX_TOKENS));
        map.insert("response_format".to_string(), json!({"type": "json_object"}));
        if include_temperature {
            map.insert(
                "temperature".to_string(),
                json!(super::DEFAULT_TEMPERATURE),
            );
        }
        serde_json::Value::Object(map)
    }

    fn remove_top_level_field(body: &mut serde_json::Value, key: &str) -> bool {
        match body {
            serde_json::Value::Object(map) => map.remove(key).is_some(),
            _ => false,
        }
    }

    fn should_retry_without_temperature(err: &anyhow::Error) -> bool {
        let msg = err.to_string().to_lowercase();
        if !msg.contains("temperature") {
            return false;
        }

        msg.contains("api error (400)")
            || msg.contains("unsupported")
            || msg.contains("not support")
            || msg.contains("not allowed")
            || msg.contains("unknown parameter")
            || msg.contains("invalid parameter")
            || msg.contains("invalid_request_error")
            || msg.contains("extra fields not permitted")
            || msg.contains("不支持")
            || msg.contains("未知参数")
            || msg.contains("无效参数")
    }

    async fn send_request_with_compat(&self, mut body: serde_json::Value) -> Result<String> {
        let first_err = match self.send_request(body.clone()).await {
            Ok(resp) => return Ok(resp),
            Err(err) => err,
        };

        if Self::should_retry_without_temperature(&first_err)
            && Self::remove_top_level_field(&mut body, "temperature")
        {
            return self.send_request(body).await.with_context(|| {
                format!(
                    "Compatibility retry without `temperature` failed after initial error: {}",
                    first_err
                )
            });
        }

        Err(first_err)
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

            match super::handle_error_response(status, &text, attempt, "OpenAI") {
                Ok(msg) => {
                    last_err = Some(msg);
                    super::backoff_delay(attempt).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(super::bail_last_err(last_err, "OpenAI request failed"))
    }
}

#[async_trait]
impl LlmBackend for OpenAiBackend {
    async fn chat(&self, system: &str, user: &str) -> Result<String> {
        let messages = vec![
            json!({"role": "system", "content": system}),
            json!({"role": "user", "content": user}),
        ];
        let body = self.build_body(messages, !self.should_omit_temperature_by_default());
        self.send_request_with_compat(body).await
    }

    async fn chat_with_history(&self, system: &str, messages: &[Message]) -> Result<String> {
        let mut msgs = vec![json!({"role": "system", "content": system})];
        for m in messages {
            msgs.push(json!({"role": m.role, "content": m.content}));
        }
        let body = self.build_body(msgs, !self.should_omit_temperature_by_default());
        self.send_request_with_compat(body).await
    }
}

#[cfg(test)]
mod tests {
    use super::OpenAiBackend;
    use crate::config::OpenAiConfig;
    use serde_json::json;

    fn backend(base_url: Option<&str>, model: &str) -> OpenAiBackend {
        OpenAiBackend::new(OpenAiConfig {
            api_key: "sk-test".to_string(),
            model: model.to_string(),
            base_url: base_url.map(ToString::to_string),
        })
    }

    #[test]
    fn build_url_appends_v1_for_plain_base_url() {
        let b = backend(Some("https://api.deepseek.com"), "deepseek-chat");
        assert_eq!(
            b.build_url(),
            "https://api.deepseek.com/v1/chat/completions"
        );
    }

    #[test]
    fn build_url_avoids_duplicate_v1_when_present() {
        let b = backend(Some("https://openrouter.ai/api/v1"), "auto");
        assert_eq!(
            b.build_url(),
            "https://openrouter.ai/api/v1/chat/completions"
        );
    }

    #[test]
    fn build_url_accepts_full_chat_completions_url() {
        let b = backend(
            Some("https://example.com/custom/v1/chat/completions"),
            "custom-model",
        );
        assert_eq!(
            b.build_url(),
            "https://example.com/custom/v1/chat/completions"
        );
    }

    #[test]
    fn kimi_k25_model_omits_temperature_by_default() {
        let b = backend(Some("https://api.moonshot.cn"), "kimi-k2.5");
        assert!(b.should_omit_temperature_by_default());
    }

    #[test]
    fn kimi_k2_model_keeps_temperature_by_default() {
        let b = backend(Some("https://api.moonshot.cn"), "kimi-k2-0711-preview");
        assert!(!b.should_omit_temperature_by_default());
    }

    #[test]
    fn non_kimi_model_keeps_temperature_by_default() {
        let b = backend(Some("https://api.openai.com"), "gpt-4o-mini");
        assert!(!b.should_omit_temperature_by_default());
    }

    #[test]
    fn remove_top_level_field_removes_temperature() {
        let mut body = json!({"model":"x", "temperature":0.1});
        assert!(OpenAiBackend::remove_top_level_field(&mut body, "temperature"));
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn retry_without_temperature_only_for_unsupported_temperature_errors() {
        let err = anyhow::anyhow!(
            "OpenAI API error (400): {\"error\":\"unsupported parameter: temperature\"}"
        );
        assert!(OpenAiBackend::should_retry_without_temperature(&err));

        let other = anyhow::anyhow!("OpenAI API error (400): {\"error\":\"invalid model\"}");
        assert!(!OpenAiBackend::should_retry_without_temperature(&other));
    }
}
