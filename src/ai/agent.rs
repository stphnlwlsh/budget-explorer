//! Tool-calling agent loop with text-based function calling
//!
//! Gemma 4 (and similar models) don't support native tool calling via
//! the tool_calls field. Instead, we instruct the model to output
//! JSON in a specific format within its text response, which we parse
//! and execute.

use crate::ai::{LLMProvider, Message};
use crate::tools::{ToolCall, ToolRegistry, ToolResult};
use serde::Deserialize;
use std::sync::Arc;

/// Tool call parsed from LLM response.
#[derive(Debug, Clone, Deserialize)]
pub struct ParsedToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Maximum number of tool-calling iterations before giving up.
const MAX_ITERATIONS: u32 = 10;

/// Map technical tool names to human-friendly names.
fn friendly_tool_name(tool_name: &str) -> &'static str {
    match tool_name {
        "get_plans" => "Fetching budget plans",
        "get_plan" => "Fetching budget plan details",
        "get_accounts" => "Fetching accounts",
        "get_categories" => "Fetching categories",
        "get_payees" => "Fetching payees",
        "get_transactions" => "Fetching all transactions",
        "get_transactions_by_month" => "Fetching month transactions",
        "get_month" => "Fetching month summary",
        "get_scheduled_transactions" => "Fetching scheduled transactions",
        _ => "Running tool",
    }
}

/// Print progress indicator to stderr.
fn print_progress(msg: &str) {
    eprintln!("\n→ {}", msg);
}

/// Agent for handling tool-calling conversations.
pub struct Agent {
    registry: Arc<ToolRegistry>,
    llm: Arc<dyn LLMProvider>,
}

impl Agent {
    pub fn new(registry: Arc<ToolRegistry>, llm: Arc<dyn LLMProvider>) -> Self {
        Self { registry, llm }
    }

    /// Run the agent loop with the given user message and system prompt.
    pub async fn run(&self, user_message: &str, system_prompt: &str) -> Result<String, crate::ai::LLMError> {
        let mut messages = vec![
            Message {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: user_message.to_string(),
            },
        ];

        for _ in 0..MAX_ITERATIONS {
            // Get response from LLM
            let response = self.llm.chat(messages.clone()).await?;
            
            // Check if the response contains a tool call
            if let Some(tool_call) = self.parse_tool_call(&response) {
                // Print progress indicator
                print_progress(friendly_tool_name(&tool_call.name));
                
                // Execute the tool (sync)
                let result = self.registry.execute(&tool_call.name, tool_call.arguments);
                
                // Add assistant message and tool result to conversation
                messages.push(Message {
                    role: "assistant".to_string(),
                    content: response.clone(),
                });
                messages.push(Message {
                    role: "user".to_string(),
                    content: format!(
                        "Tool result for {}:\n{}",
                        tool_call.name,
                        self.format_tool_result(&result)
                    ),
                });
            } else {
                // No tool call - return the response as-is
                return Ok(response);
            }
        }

        Err(crate::ai::LLMError::Api(
            "Max tool-calling iterations reached".to_string()
        ))
    }

    /// Parse a tool call from the LLM response text.
    /// 
    /// Looks for JSON in the format: {"name": "...", "arguments": {...}}
    /// or {"tool_name": "...", "arguments": {...}}
    /// The JSON may be wrapped in markdown code blocks or plain text.
    fn parse_tool_call(&self, text: &str) -> Option<ParsedToolCall> {
        // Try to find JSON in the text
        // First, try to find a code block with JSON
        let json_text = self.extract_json(text)?;
        
        // Parse the JSON
        let parsed: serde_json::Value = serde_json::from_str(&json_text).ok()?;
        
        // Extract name - check both "name" and "tool_name"
        let name = parsed.get("name")
            .or_else(|| parsed.get("tool_name"))?
            .as_str()?
            .to_string();
        let arguments = parsed.get("arguments")?.clone();
        
        Some(ParsedToolCall { name, arguments })
    }

    /// Extract JSON from text, handling various formats.
    fn extract_json(&self, text: &str) -> Option<String> {
        // Try direct parse first
        if serde_json::from_str::<serde_json::Value>(text).is_ok() {
            return Some(text.to_string());
        }

        // Try to find JSON in code blocks
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('{') && trimmed.ends_with('}')
                && serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
            {
                return Some(trimmed.to_string());
            }
        }

        // Try to find code blocks with json
        let mut in_code_block = false;
        let mut json_buffer = String::new();
        
        for line in text.lines() {
            if line.trim().starts_with("```json") {
                in_code_block = true;
                json_buffer.clear();
                continue;
            }
            if line.trim() == "```" && in_code_block {
                if serde_json::from_str::<serde_json::Value>(&json_buffer).is_ok() {
                    return Some(json_buffer.clone());
                }
                in_code_block = false;
                json_buffer.clear();
            }
            if in_code_block {
                json_buffer.push_str(line);
                json_buffer.push('\n');
            }
        }

        // Try to find the first { ... } block
        if let Some(start) = text.find('{') {
            let mut depth = 0;
            for (i, c) in text[start..].chars().enumerate() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            let json_str = &text[start..=start + i];
                            if serde_json::from_str::<serde_json::Value>(json_str).is_ok() {
                                return Some(json_str.to_string());
                            }
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }

        None
    }

    /// Format a tool result for the LLM.
    fn format_tool_result(&self, result: &ToolResult) -> String {
        if result.success {
            result.data.clone().unwrap_or_else(|| "No data returned".to_string())
        } else if let Some(ref error) = result.error {
            format!("Error: {}", error)
        } else {
            "Unknown error".to_string()
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{LLMError, LLMProvider};
    use crate::tools::ToolResult;
    use async_trait::async_trait;
    use std::sync::Arc;

    // ========================================================================
    // Mock LLM Provider for tests
    // ========================================================================

    pub struct MockLLM {
        response: String,
    }

    impl MockLLM {
        pub fn new(response: &str) -> Self {
            Self { response: response.to_string() }
        }
    }

    #[async_trait]
    impl LLMProvider for MockLLM {
        async fn chat(&self, _messages: Vec<Message>) -> Result<String, LLMError> {
            Ok(self.response.clone())
        }
    }

    // Helper function to create an Agent with mock dependencies
    fn make_test_agent(response: &str) -> Agent {
        let client = crate::ynab::Client::new("test-token");
        let client = Arc::new(client);
        let registry = Arc::new(crate::tools::ToolRegistry::new(client));
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLM::new(response));
        Agent::new(registry, llm)
    }

    // ========================================================================
    // extract_json tests - Positive cases
    // ========================================================================

    #[test]
    fn extract_json_parses_plain_json() {
        // Arrange
        let agent = make_test_agent("");
        let text = r#"{"name": "get_plans", "arguments": {}}"#;
        // Act
        let result = agent.extract_json(text);
        // Assert
        assert!(result.is_some());
    }

    #[test]
    fn extract_json_parses_json_in_markdown_code_block() {
        // Arrange
        let agent = make_test_agent("");
        let text = r#"Here is the tool call:
```json
{"name": "get_plans", "arguments": {}}
```
 "#;
        // Act
        let result = agent.extract_json(text);
        // Assert
        let json = result.unwrap();
        assert!(json.contains("get_plans"));
    }

    #[test]
    fn extract_json_finds_json_inside_text() {
        // Arrange
        let agent = make_test_agent("");
        let text = r#"I need to call {"name": "get_categories", "arguments": {"plan_id": "abc123"}} to get your categories."#;
        // Act
        let result = agent.extract_json(text);
        // Assert
        assert!(result.is_some());
    }

    #[test]
    fn extract_json_handles_nested_objects() {
        // Arrange
        let agent = make_test_agent("");
        let text = r#"{"name": "get_transactions", "arguments": {"plan_id": "plan-1", "start_date": "2026-04-01", "filters": {"category": "groceries"}}}"#;
        // Act
        let result = agent.extract_json(text);
        // Assert
        assert!(result.is_some());
    }

    // ========================================================================
    // extract_json tests - Negative cases
    // ========================================================================

    #[test]
    fn extract_json_returns_none_for_plain_text() {
        // Arrange
        let agent = make_test_agent("");
        let text = "This is just plain text with no JSON.";
        // Act
        let result = agent.extract_json(text);
        // Assert
        assert!(result.is_none());
    }

    #[test]
    fn extract_json_returns_none_for_incomplete_json() {
        // Arrange
        let agent = make_test_agent("");
        let text = r#"{"name": "test", "arguments": {"foo"#;
        // Act
        let result = agent.extract_json(text);
        // Assert
        assert!(result.is_none());
    }

    #[test]
    fn extract_json_handles_multiline_code_block() {
        // Arrange
        let agent = make_test_agent("");
        let text = r#"I'll get your transactions:
```json
{
  "name": "get_transactions",
  "arguments": {
    "plan_id": "plan-1"
  }
}
```
Let me know if you need anything else!"#;
        // Act
        let result = agent.extract_json(text);
        // Assert
        assert!(result.is_some());
    }

    #[test]
    fn extract_json_returns_none_for_unclosed_braces() {
        // Arrange
        let agent = make_test_agent("");
        let text = r#"{"name": "test", "arguments": {"foo": "bar"}"#;
        // Act
        let result = agent.extract_json(text);
        // Assert
        assert!(result.is_none());
    }

    // ========================================================================
    // format_tool_result tests - Positive cases
    // ========================================================================

    #[test]
    fn format_tool_result_returns_data_on_success() {
        // Arrange
        let agent = make_test_agent("");
        let result = ToolResult {
            tool: "get_plans".to_string(),
            success: true,
            data: Some(r#"[{"id": "plan-1", "name": "My Budget"}]"#.to_string()),
            error: None,
        };
        // Act
        let formatted = agent.format_tool_result(&result);
        // Assert
        assert!(formatted.contains("plan-1"));
    }

    #[test]
    fn format_tool_result_returns_no_data_message_when_empty() {
        // Arrange
        let agent = make_test_agent("");
        let result = ToolResult {
            tool: "get_plans".to_string(),
            success: true,
            data: None,
            error: None,
        };
        // Act
        let formatted = agent.format_tool_result(&result);
        // Assert
        assert_eq!(formatted, "No data returned");
    }

    #[test]
    fn format_tool_result_returns_error_on_failure() {
        // Arrange
        let agent = make_test_agent("");
        let result = ToolResult {
            tool: "get_plans".to_string(),
            success: false,
            data: None,
            error: Some("Network error: connection refused".to_string()),
        };
        // Act
        let formatted = agent.format_tool_result(&result);
        // Assert
        assert!(formatted.contains("Error"));
    }

    #[test]
    fn format_tool_result_returns_unknown_error_when_no_message() {
        // Arrange
        let agent = make_test_agent("");
        let result = ToolResult {
            tool: "get_plans".to_string(),
            success: false,
            data: None,
            error: None,
        };
        // Act
        let formatted = agent.format_tool_result(&result);
        // Assert
        assert_eq!(formatted, "Unknown error");
    }
}
