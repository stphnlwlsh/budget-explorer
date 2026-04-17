//! Ollama LLM provider with native tool calling support

#![allow(dead_code, unused_imports, unused_variables)]

use crate::ai::{LLMError, LLMProvider, LLMResponse, Message, ToolCall};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

/// Ollama provider for local LLM inference with tool support.
pub struct OllamaProvider {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

/// Ollama API response structure.
#[derive(Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
    done: bool,
}

/// Ollama message structure.
#[derive(Deserialize)]
struct OllamaMessage {
    content: String,
    /// Tool calls (if any) - present when model requests tool use
    #[serde(default)]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

/// Tool call from Ollama in native format.
#[derive(Deserialize)]
struct OllamaToolCall {
    function: OllamaToolFunction,
}

/// Function details from Ollama tool call.
#[derive(Deserialize)]
struct OllamaToolFunction {
    name: String,
    arguments: Value,
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

    /// Send a chat request with optional tools.
    async fn chat_request_with_tools(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<Value>>,
    ) -> Result<LLMResponse, LLMError> {
        let url = format!("{}/api/chat", self.base_url);

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages.into_iter().map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content
                })
            }).collect::<Vec<_>>(),
            "stream": false,
            // Recommended sampling parameters for Gemma 4
            "options": {
                "temperature": 1.0,
                "top_p": 0.95,
                "top_k": 64
            }
        });

        // Add tools if provided
        if let Some(tools) = tools {
            body["tools"] = serde_json::json!(tools);
        }

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

        if ollama_resp.message.content.is_empty() && ollama_resp.message.tool_calls.is_none() {
            return Err(LLMError::EmptyResponse);
        }

        // Convert Ollama tool calls to our format
        let tool_calls = ollama_resp.message.tool_calls.map(|calls| {
            calls
                .into_iter()
                .map(|call| ToolCall {
                    name: call.function.name,
                    arguments: call.function.arguments,
                })
                .collect()
        });

        Ok(LLMResponse {
            content: ollama_resp.message.content,
            tool_calls,
        })
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    /// Send a chat request without tools (text-only response).
    async fn chat(&self, messages: Vec<Message>) -> Result<String, LLMError> {
        let response = self.chat_request_with_tools(messages, None).await?;
        Ok(response.content)
    }

    /// Send a chat request with tools and get structured response.
    async fn chat_with_tools(
        &self,
        messages: Vec<Message>,
        tools: Vec<Value>,
    ) -> Result<LLMResponse, LLMError> {
        self.chat_request_with_tools(messages, Some(tools)).await
    }
}
