//! AI Module - LLM providers and agent

#![allow(dead_code, unused_imports, unused_variables)]

pub mod agent;
pub mod ollama;

pub use agent::Agent;
pub use ollama::OllamaProvider;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::profile::{AdviceStyle, Tone, UserProfile};

/// Message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// LLM Provider trait.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat(&self, messages: Vec<Message>) -> Result<String, LLMError>;
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

    format!(r#"You are a budget advisor. Answer questions about spending using the available tools.

{goals_section}{concerns_section}
IMPORTANT RULES:
- Plain text only, NO markdown, NO tables
- 1-2 sentences max
- Use dollars like "$1,234.56"

HOW TO ANSWER SPENDING QUESTIONS:
1. Get the plan ID with get_plans
2. Get transactions with get_transactions (includes payee_name field)
3. Filter the transactions by payee name in your head
4. Calculate total and respond

TOOL CALL FORMAT - output ONLY JSON:
{{"name": "get_plans", "arguments": {{}}}}
{{"name": "get_transactions", "arguments": {{"plan_id": "YOUR_PLAN_ID"}}}}

AVAILABLE TOOLS:
{tools_json}

EXAMPLE:
User: "how much at Apple?"
You: {{"name": "get_plans", "arguments": {{}}}}
(next): {{"name": "get_transactions", "arguments": {{"plan_id": "abc123"}}}}
Response: "You spent $847.23 at Apple this year (12 transactions)."

TONE: {tone_instruction}
STYLE: {style_instruction}"#,
        goals_section = goals_section,
        concerns_section = concerns_section,
        tone_instruction = tone_instruction,
        style_instruction = style_instruction,
        tools_json = tools_json
    )
}
