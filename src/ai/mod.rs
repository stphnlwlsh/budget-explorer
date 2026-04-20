//! AI Module - LLM providers and agent
//!
//! GAME Framework implementation:
//! - G (Goals): Task goals that guide agent behavior
//! - A (Actions): Tool registry (in tools/mod.rs)
//! - M (Memory): Conversation memory (in agent.rs)
//! - E (Environment): YNAB API abstraction

#![allow(dead_code, unused_imports, unused_variables)]

pub mod agent;
pub mod ollama;
pub mod goal;

pub use agent::Agent;
pub use ollama::OllamaProvider;

// GAME Framework exports from goal.rs
pub use crate::ai::goal::{Goal, standard_goals, merge_goals, Environment, YnabEnvironment};

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::profile::{AdviceStyle, Tone, UserProfile};

// ============================================================================
// GAME Framework (defined in goal.rs)
// ============================================================================
// Goals and Environment are defined in goal.rs module
// Re-exported at module level for convenience

/// Message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// A tool call returned by the LLM (native format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Name of the tool to call.
    pub name: String,
    /// Arguments to pass to the tool.
    pub arguments: Value,
}

/// LLM response with optional tool calls.
#[derive(Debug, Clone)]
pub struct LLMResponse {
    /// Text content from the LLM.
    pub content: String,
    /// Optional tool calls requested by the LLM.
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl LLMResponse {
    /// Check if response has tool calls.
    pub fn has_tool_calls(&self) -> bool {
        self.tool_calls.as_ref().is_some_and(|c| !c.is_empty())
    }
}

/// LLM Provider trait.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Send a chat request without tools (text-only response).
    async fn chat(&self, messages: Vec<Message>) -> Result<String, LLMError>;

    /// Send a chat request with tools and get structured response.
    async fn chat_with_tools(
        &self,
        messages: Vec<Message>,
        tools: Vec<Value>,
    ) -> Result<LLMResponse, LLMError>;
}

/// LLM Errors.
#[derive(Debug)]
pub enum LLMError {
    Network(String),
    Auth(String),
    Api(String),
    Parse(String),
    EmptyResponse,
}

impl std::fmt::Display for LLMError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LLMError::Network(e) => write!(f, "Network error: {}", e),
            LLMError::Auth(e) => write!(f, "Auth error: {}", e),
            LLMError::Api(e) => write!(f, "API error: {}", e),
            LLMError::Parse(e) => write!(f, "Parse error: {}", e),
            LLMError::EmptyResponse => write!(f, "Empty response from LLM"),
        }
    }
}

impl std::error::Error for LLMError {}

/// Build system prompt based on user profile and available tools.
pub fn build_system_prompt(profile: Option<&UserProfile>, tools_json: &str) -> String {
    let (goals_section, concerns_section) = if let Some(p) = profile {
        let goals = if p.goals.is_empty() {
            "Track spending and understand spending patterns".to_string()
        } else {
            p.goals.join(", ")
        };
        let concerns = if p.concerns.is_empty() {
            "Reduce unnecessary spending".to_string()
        } else {
            p.concerns.join(", ")
        };
        (
            format!("Your goals: {}. ", goals),
            format!("Key concerns: {}. ", concerns),
        )
    } else {
        (
            "Your goal: Help users understand their budget and spending.".to_string(),
            String::new(),
        )
    };

    let (tone_instruction, style_instruction) = if let Some(p) = profile {
        let tone = match p.tone {
            Tone::Friendly => "Be warm, encouraging, and supportive. Use phrases like 'Great job!' or 'You've got this!'",
            Tone::Direct => "Be direct and to the point. State facts without extra fluff.",
            Tone::Professional => "Be formal and professional. Use clear, business-like language.",
        };
        let style = match p.advice_style {
            AdviceStyle::Summary => "Keep responses SHORT. 1-3 sentences max. Just give the key number or finding.",
            AdviceStyle::Detailed => "Provide thorough analysis. Include context, comparisons, and reasoning.",
            AdviceStyle::ActionItems => "Focus on specific next steps. Use bullet points or numbered lists of actions.",
        };
        (tone.to_string(), style.to_string())
    } else {
        ("Be helpful and friendly.".to_string(), "Keep responses concise and informative.".to_string())
    };

    format!(r#"You are a helpful budget advisor. Your role is to retrieve budget data using tools, then summarize it clearly based on what the user asks.

{goals_section}{concerns_section}
IMPORTANT RULES:
1. ONLY use tools to GET data - do not make up or estimate numbers
2. ONLY summarize what the tools return - no calculations beyond basic addition/averages
3. Plain text only, NO markdown, NO tables
4. Use dollars like "$1,234.56"

WORKFLOW:
1. Call get_plans to find the plan ID
2. Call the appropriate tool to fetch the data (search_payee_transactions, get_transactions, get_month, etc.)
3. Summarize the data in the style requested by the user

AVAILABLE TOOLS:
{tools_json}

TONE: {tone_instruction}
STYLE: {style_instruction}"#,
        goals_section = goals_section,
        concerns_section = concerns_section,
        tone_instruction = tone_instruction,
        style_instruction = style_instruction,
        tools_json = tools_json
    )
}
