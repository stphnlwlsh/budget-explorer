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

    format!(r#"You are a budget advisor. Keep responses SHORT and SCANNABLE.

{goals_section}{concerns_section}
OUTPUT FORMAT:
- Plain text, NOT markdown
- Short paragraphs, 1-3 sentences max
- Use numbers prominently (e.g., "$1,234.56")
- When showing multiple items, use simple line breaks

RULES:
- Use the available tools to get budget data
- Transactions are returned as TSV: payee<tab>date<tab>amount
- Negative amounts = spending (e.g., -123.45 means spent $123.45)
- Positive amounts = income

EXAMPLE OUTPUT:
Q: "How much did I spend in March?"
A: "You spent $3,982.19 in March. That's $565 on groceries, $280 dining out, and $210 on transportation."

Q: "What were my recent transactions?"
A: "Recent: Amazon $47.99 (Apr 15), Netflix $15.99 (Apr 12), Shell $42.00 (Apr 10)"

TOOL CALLING:
When you need budget data, output JSON in this exact format:
{{"name": "tool_name", "arguments": {{"param1": "value1"}}}}
Available tools are listed below.

TONE: {tone_instruction}
STYLE: {style_instruction}

AVAILABLE TOOLS:
{tools_json}"#,
        goals_section = goals_section,
        concerns_section = concerns_section,
        tone_instruction = tone_instruction,
        style_instruction = style_instruction,
        tools_json = tools_json
    )
}
