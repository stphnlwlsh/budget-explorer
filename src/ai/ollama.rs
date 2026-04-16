//! Ollama LLM provider

#![allow(dead_code, unused_imports, unused_variables)]

use crate::ai::{LLMError, LLMProvider, Message};
use async_trait::async_trait;
use serde::Deserialize;

/// Ollama provider for local LLM inference.
pub struct OllamaProvider {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
    done: bool,
}

#[derive(Deserialize)]
struct OllamaMessage {
    content: String,
}

impl OllamaProvider {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            model: model.to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn from_env() -> Result<Self, String> {
        let base_url = std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let model = std::env::var("OLLAMA_MODEL")
            .map_err(|_| "OLLAMA_MODEL not set")?;
        Ok(Self::new(&base_url, &model))
    }

    /// Send a chat request and get the complete response.
    async fn chat_request(&self, messages: Vec<Message>) -> Result<String, LLMError> {
        let url = format!("{}/api/chat", self.base_url);
        
        let ollama_messages: Vec<serde_json::Value> = messages
            .into_iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content
                })
            })
            .collect();

        let body = serde_json::json!({
            "model": self.model,
            "messages": ollama_messages,
            "stream": false
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| LLMError::Network(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(LLMError::Api(format!("{}: {}", status, body)));
        }

        let ollama_resp: OllamaResponse = resp
            .json()
            .await
            .map_err(|e| LLMError::Parse(e.to_string()))?;

        if ollama_resp.message.content.is_empty() {
            return Err(LLMError::EmptyResponse);
        }

        Ok(ollama_resp.message.content)
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn chat(&self, messages: Vec<Message>) -> Result<String, LLMError> {
        self.chat_request(messages).await
    }
}
