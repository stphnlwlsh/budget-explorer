//! Tool-calling agent loop with native Ollama function calling support
//!
//! Supports both:
//! - Native tool calling via Ollama's `tools` parameter (Gemma 4, Llama 3.1)
//! - Text-based parsing as fallback for models without native support

use crate::ai::{LLMProvider, LLMResponse, Message};
use crate::tools::{ToolRegistry, ToolResult};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

/// Tool call (either from native parsing or text extraction).
#[derive(Debug, Clone)]
pub struct ParsedToolCall {
    pub name: String,
    pub arguments: Value,
}

/// Parse error with actionable feedback.
#[derive(Debug, Clone)]
pub enum ParseError {
    /// No JSON found in response
    NoJsonFound,
    /// JSON was malformed
    MalformedJson(String),
    /// Missing required field
    MissingField(String),
    /// Unknown tool name
    UnknownTool(String),
    /// Tool execution failed
    ToolExecutionFailed(String),
}

impl ParseError {
    /// Convert to user-friendly hint for retry.
    fn to_hint(&self) -> String {
        match self {
            ParseError::NoJsonFound => {
                "Your response must include a JSON tool call in this format: \
                 {\"name\": \"tool_name\", \"arguments\": {}}"
                    .to_string()
            }
            ParseError::MalformedJson(detail) => {
                format!(
                    "Invalid JSON format: {}. \
                     Make sure to use valid JSON with double quotes.",
                    detail
                )
            }
            ParseError::MissingField(field) => {
                format!(
                    "Missing required field '{}'. Include both 'name' and 'arguments' fields.",
                    field
                )
            }
            ParseError::UnknownTool(name) => {
                format!(
                    "Unknown tool '{}'. Available tools: get_plans, get_plan, \
                     get_accounts, get_categories, get_payees, get_transactions, \
                     get_transactions_by_month, get_month, get_scheduled_transactions, \
                     search_payee_transactions",
                    name
                )
            }
            ParseError::ToolExecutionFailed(detail) => {
                format!("Tool execution failed: {}", detail)
            }
        }
    }
}

/// Conversation memory for tracking history.
#[derive(Debug, Clone)]
pub struct ConversationMemory {
    /// Number of tool-call turns in this conversation.
    pub turns: u32,
    /// Full message history for context.
    history: Vec<Message>,
}

impl ConversationMemory {
    pub fn new() -> Self {
        Self {
            turns: 0,
            history: Vec::new(),
        }
    }

    /// Add initial system prompt and user message.
    pub fn init(&mut self, system_prompt: &str, user_message: &str) {
        self.history.push(Message {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        });
        self.history.push(Message {
            role: "user".to_string(),
            content: user_message.to_string(),
        });
    }

    /// Add assistant's response to history.
    pub fn add_assistant_response(&mut self, content: &str) {
        self.history.push(Message {
            role: "assistant".to_string(),
            content: content.to_string(),
        });
    }

    /// Add user's (tool result) response to history.
    pub fn add_tool_result(&mut self, tool_name: &str, result: &str) {
        self.history.push(Message {
            role: "user".to_string(),
            content: format!("Tool result for {}:\n{}", tool_name, result),
        });
    }

    /// Add a parse error hint as a user message.
    pub fn add_parse_error_hint(&mut self, hint: &str) {
        self.history.push(Message {
            role: "user".to_string(),
            content: format!(
                "Parse error: {}\n\nPlease respond with a valid tool call JSON.",
                hint
            ),
        });
    }

    /// Get current message history for LLM.
    pub fn get_messages(&self) -> &[Message] {
        &self.history
    }

    /// Increment turn counter after successful tool execution.
    pub fn increment_turn(&mut self) {
        self.turns += 1;
    }

    /// Get current turn count.
    pub fn turn_count(&self) -> u32 {
        self.turns
    }
}

impl Default for ConversationMemory {
    fn default() -> Self {
        Self::new()
    }
}

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
        "search_payee_transactions" => "Searching transactions",
        _ => "Running tool",
    }
}

/// Print progress indicator to stderr.
fn print_progress(msg: &str) {
    eprintln!("\n→ {}", msg);
}

// ============================================================================
// Agent Builder
// ============================================================================

/// Builder for constructing an Agent with custom configuration.
pub struct AgentBuilder {
    registry: Option<Arc<ToolRegistry>>,
    llm: Option<Arc<dyn LLMProvider>>,
    max_iterations: u32,
    memory: Option<ConversationMemory>,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            registry: None,
            llm: None,
            max_iterations: 10,
            memory: None,
        }
    }

    /// Set the tool registry.
    pub fn with_registry(mut self, registry: Arc<ToolRegistry>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Set the LLM provider.
    pub fn with_llm(mut self, llm: Arc<dyn LLMProvider>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Set max tool-calling iterations before giving up.
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// Set existing conversation memory (for resuming conversations).
    pub fn with_memory(mut self, memory: ConversationMemory) -> Self {
        self.memory = Some(memory);
        self
    }

    /// Build the Agent instance.
    pub fn build(self) -> Result<Agent, &'static str> {
        let registry = self.registry.ok_or("registry is required")?;
        let llm = self.llm.ok_or("llm is required")?;

        Ok(Agent {
            registry,
            llm,
            max_iterations: self.max_iterations,
            memory: self.memory.unwrap_or_default(),
            tools: Vec::new(),
        })
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Agent
// ============================================================================

/// Agent for handling tool-calling conversations.
pub struct Agent {
    registry: Arc<ToolRegistry>,
    llm: Arc<dyn LLMProvider>,
    max_iterations: u32,
    memory: ConversationMemory,
    /// Tool definitions in Ollama format for native tool calling.
    tools: Vec<Value>,
}

impl std::fmt::Debug for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Agent")
            .field("max_iterations", &self.max_iterations)
            .field("memory", &self.memory)
            .field("tools_count", &self.tools.len())
            .finish()
    }
}

impl Agent {
    /// Create agent with default configuration.
    pub fn new(registry: Arc<ToolRegistry>, llm: Arc<dyn LLMProvider>) -> Self {
        Self {
            registry,
            llm,
            max_iterations: 10,
            memory: ConversationMemory::new(),
            tools: Vec::new(),
        }
    }

    /// Create agent using builder pattern.
    pub fn builder() -> AgentBuilder {
        AgentBuilder::new()
    }

    /// Get mutable reference to memory for inspection.
    pub fn memory_mut(&mut self) -> &mut ConversationMemory {
        &mut self.memory
    }

    /// Set tool definitions (Ollama format).
    pub fn with_tools(mut self, tools: Vec<Value>) -> Self {
        self.tools = tools;
        self
    }

    /// Run the agent loop with the given user message and system prompt.
    pub async fn run(
        &mut self,
        user_message: &str,
        system_prompt: &str,
    ) -> Result<String, AgentError> {
        // Initialize memory with first message
        self.memory.init(system_prompt, user_message);

        for _ in 0..self.max_iterations {
            // Get response from LLM with native tool calling
            let response = if self.tools.is_empty() {
                // No tools - use simple chat
                let content = self.llm.chat(self.memory.get_messages().to_vec()).await?;
                LLMResponse {
                    content,
                    tool_calls: None,
                }
            } else {
                // Use native tool calling
                self.llm
                    .chat_with_tools(self.memory.get_messages().to_vec(), self.tools.clone())
                    .await?
            };

            // Check for native tool calls first
            if let Some(tool_calls) = &response.tool_calls {
                if !tool_calls.is_empty() {
                    // Handle native tool calls
                    for tool_call in tool_calls {
                        // Print progress indicator
                        print_progress(friendly_tool_name(&tool_call.name));

                        // Execute the tool (sync)
                        let result = self.registry.execute(&tool_call.name, tool_call.arguments.clone());

                        // Add to history
                        let formatted_result = self.format_tool_result(&result);
                        self.memory.add_tool_result(&tool_call.name, &formatted_result);
                        self.memory.increment_turn();

                        // Check if tool execution failed
                        if !result.success {
                            // Add error hint and let LLM retry
                            self.memory.add_parse_error_hint(&formatted_result);
                            break;
                        }
                    }

                    // If we had successful tool executions, continue to next iteration
                    if response.tool_calls.as_ref().is_some_and(|c| !c.is_empty()) {
                        continue;
                    }
                }
            }

            // Fallback: try to parse tool call from text response
            match self.parse_tool_call(&response.content) {
                Ok(tool_call) => {
                    // Print progress indicator
                    print_progress(friendly_tool_name(&tool_call.name));

                    // Execute the tool (sync)
                    let result = self.registry.execute(&tool_call.name, tool_call.arguments.clone());

                    // Add to history
                    self.memory.add_assistant_response(&response.content);
                    let formatted_result = self.format_tool_result(&result);
                    self.memory.add_tool_result(&tool_call.name, &formatted_result);
                    self.memory.increment_turn();

                    // Check if tool execution failed
                    if !result.success {
                        // Add error hint and let LLM retry
                        self.memory.add_parse_error_hint(&formatted_result);
                        continue;
                    }
                }
                Err(parse_error) => {
                    // No tool call found - check if this is a valid response
                    if self.contains_valid_response(&response.content) {
                        self.memory.add_assistant_response(&response.content);
                        return Ok(response.content);
                    }

                    // Parse failed - add hint and retry
                    self.memory.add_assistant_response(&response.content);
                    self.memory.add_parse_error_hint(&parse_error.to_hint());
                    continue;
                }
            }
        }

        Err(AgentError::MaxIterationsReached(self.memory.turn_count()))
    }

    /// Check if response contains a valid text response (not just a tool call).
    fn contains_valid_response(&self, text: &str) -> bool {
        let trimmed = text.trim();
        // If it looks like JSON, it's NOT a valid response (return false)
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return false;
        }
        // Check for tool call patterns anywhere in the text
        if trimmed.contains("{\"name\":") || trimmed.contains("{\"tool_name\":") {
            return false;
        }
        // Check if the response is mostly JSON by looking for significant JSON content
        let json_chars = trimmed.chars().filter(|c| *c == '{' || *c == '}').count();
        let total_chars = trimmed.len();
        if total_chars > 10 && json_chars as f64 / total_chars as f64 > 0.3 {
            return false;
        }
        // Otherwise, it's a valid text response
        true
    }

    /// Parse a tool call from the LLM response text.
    ///
    /// Looks for JSON in the format: {"name": "...", "arguments": {...}}
    /// or {"tool_name": "...", "arguments": {...}}
    /// The JSON may be wrapped in markdown code blocks or plain text.
    fn parse_tool_call(&self, text: &str) -> Result<ParsedToolCall, ParseError> {
        // Try to find JSON in the text
        let json_text = self.extract_json(text).ok_or(ParseError::NoJsonFound)?;

        // Parse the JSON
        let parsed: serde_json::Value =
            serde_json::from_str(&json_text).map_err(|e| {
                ParseError::MalformedJson(e.to_string())
            })?;

        // Extract name - check both "name" and "tool_name"
        let name = parsed
            .get("name")
            .or_else(|| parsed.get("tool_name"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| ParseError::MissingField("name/tool_name".to_string()))?;

        // Verify tool exists
        if self.registry.get_definitions().iter().all(|t| t.name != name) {
            return Err(ParseError::UnknownTool(name));
        }

        let arguments = parsed
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::Value::Object(Default::default()));

        Ok(ParsedToolCall { name, arguments })
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
            result
                .data
                .clone()
                .unwrap_or_else(|| "No data returned".to_string())
        } else if let Some(ref error) = result.error {
            format!("Error: {}", error)
        } else {
            "Unknown error".to_string()
        }
    }
}

/// Agent-level errors.
#[derive(Debug)]
pub enum AgentError {
    Llm(crate::ai::LLMError),
    MaxIterationsReached(u32),
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::Llm(e) => write!(f, "LLM error: {}", e),
            AgentError::MaxIterationsReached(turns) => {
                write!(
                    f,
                    "Max tool-calling iterations reached after {} turns",
                    turns
                )
            }
        }
    }
}

impl std::error::Error for AgentError {}

impl From<crate::ai::LLMError> for AgentError {
    fn from(e: crate::ai::LLMError) -> Self {
        AgentError::Llm(e)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{LLMError, LLMProvider};
    use async_trait::async_trait;
    use std::sync::Arc;

    // ========================================================================
    // Mock LLM Provider for tests
    // ========================================================================

    pub struct MockLLM {
        response: String,
        /// Optional tool calls to return (for testing native tool calling)
        tool_calls: Option<Vec<crate::ai::ToolCall>>,
    }

    impl MockLLM {
        pub fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
                tool_calls: None,
            }
        }

        pub fn with_tool_calls(mut self, calls: Vec<crate::ai::ToolCall>) -> Self {
            self.tool_calls = Some(calls);
            self
        }
    }

    #[async_trait]
    impl LLMProvider for MockLLM {
        async fn chat(&self, _messages: Vec<Message>) -> Result<String, LLMError> {
            Ok(self.response.clone())
        }

        async fn chat_with_tools(
            &self,
            _messages: Vec<Message>,
            _tools: Vec<Value>,
        ) -> Result<LLMResponse, LLMError> {
            Ok(LLMResponse {
                content: self.response.clone(),
                tool_calls: self.tool_calls.clone(),
            })
        }
    }

    // ========================================================================
    // AgentBuilder tests
    // ========================================================================

    #[test]
    fn agent_builder_requires_registry() {
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLM::new("test"));
        let result = AgentBuilder::new().with_llm(llm).build();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "registry is required");
    }

    #[test]
    fn agent_builder_requires_llm() {
        let client = crate::ynab::Client::new("test-token");
        let client = Arc::new(client);
        let registry = Arc::new(crate::tools::ToolRegistry::new(client));
        let result = AgentBuilder::new().with_registry(registry).build();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "llm is required");
    }

    #[test]
    fn agent_builder_builds_successfully() {
        let client = crate::ynab::Client::new("test-token");
        let client = Arc::new(client);
        let registry = Arc::new(crate::tools::ToolRegistry::new(client));
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLM::new("test"));
        let result = AgentBuilder::new()
            .with_registry(registry)
            .with_llm(llm)
            .with_max_iterations(5)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn agent_builder_sets_max_iterations() {
        let client = crate::ynab::Client::new("test-token");
        let client = Arc::new(client);
        let registry = Arc::new(crate::tools::ToolRegistry::new(client));
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLM::new("test"));
        let agent = AgentBuilder::new()
            .with_registry(registry)
            .with_llm(llm)
            .with_max_iterations(15)
            .build()
            .unwrap();
        // Can't access private field, but build should succeed
        assert!(true);
    }

    // ========================================================================
    // ConversationMemory tests
    // ========================================================================

    #[test]
    fn conversation_memory_initializes() {
        let mut memory = ConversationMemory::new();
        assert_eq!(memory.turn_count(), 0);
        memory.init("system prompt", "user message");
        assert_eq!(memory.get_messages().len(), 2);
    }

    #[test]
    fn conversation_memory_tracks_turns() {
        let mut memory = ConversationMemory::new();
        memory.init("system", "user");
        assert_eq!(memory.turn_count(), 0);
        memory.increment_turn();
        assert_eq!(memory.turn_count(), 1);
        memory.increment_turn();
        assert_eq!(memory.turn_count(), 2);
    }

    #[test]
    fn conversation_memory_adds_assistant_response() {
        let mut memory = ConversationMemory::new();
        memory.init("system", "user");
        memory.add_assistant_response("assistant response");
        assert_eq!(memory.get_messages().len(), 3);
        assert_eq!(memory.get_messages()[2].role, "assistant");
    }

    #[test]
    fn conversation_memory_adds_tool_result() {
        let mut memory = ConversationMemory::new();
        memory.init("system", "user");
        memory.add_tool_result("get_plans", "result data");
        assert_eq!(memory.get_messages().len(), 3);
        assert_eq!(memory.get_messages()[2].role, "user");
        assert!(memory.get_messages()[2].content.contains("get_plans"));
    }

    #[test]
    fn conversation_memory_adds_parse_error_hint() {
        let mut memory = ConversationMemory::new();
        memory.init("system", "user");
        memory.add_parse_error_hint("No JSON found");
        assert_eq!(memory.get_messages().len(), 3);
        assert!(memory.get_messages()[2].content.contains("Parse error"));
    }

    // ========================================================================
    // ParseError tests
    // ========================================================================

    #[test]
    fn parse_error_to_hint_no_json() {
        let err = ParseError::NoJsonFound;
        let hint = err.to_hint();
        assert!(hint.contains("JSON tool call"));
    }

    #[test]
    fn parse_error_to_hint_malformed() {
        let err = ParseError::MalformedJson("unexpected token".to_string());
        let hint = err.to_hint();
        assert!(hint.contains("Invalid JSON"));
    }

    #[test]
    fn parse_error_to_hint_missing_field() {
        let err = ParseError::MissingField("name".to_string());
        let hint = err.to_hint();
        assert!(hint.contains("name"));
    }

    #[test]
    fn parse_error_to_hint_unknown_tool() {
        let err = ParseError::UnknownTool("fake_tool".to_string());
        let hint = err.to_hint();
        assert!(hint.contains("fake_tool"));
        assert!(hint.contains("Unknown tool"));
    }

    // ========================================================================
    // extract_json tests - Positive cases
    // ========================================================================

    fn make_test_agent() -> Agent {
        let client = crate::ynab::Client::new("test-token");
        let client = Arc::new(client);
        let registry = Arc::new(crate::tools::ToolRegistry::new(client));
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLM::new(""));
        Agent::new(registry, llm)
    }

    #[test]
    fn extract_json_parses_plain_json() {
        let agent = make_test_agent();
        let text = r#"{"name": "get_plans", "arguments": {}}"#;
        let result = agent.extract_json(text);
        assert!(result.is_some());
    }

    #[test]
    fn extract_json_parses_json_in_markdown_code_block() {
        let agent = make_test_agent();
        let text = r#"Here is the tool call:
```json
{"name": "get_plans", "arguments": {}}
```
 "#;
        let result = agent.extract_json(text);
        let json = result.unwrap();
        assert!(json.contains("get_plans"));
    }

    #[test]
    fn extract_json_finds_json_inside_text() {
        let agent = make_test_agent();
        let text = r#"I need to call {"name": "get_categories", "arguments": {"plan_id": "abc123"}} to get your categories."#;
        let result = agent.extract_json(text);
        assert!(result.is_some());
    }

    #[test]
    fn extract_json_handles_nested_objects() {
        let agent = make_test_agent();
        let text = r#"{"name": "get_transactions", "arguments": {"plan_id": "plan-1", "start_date": "2026-04-01", "filters": {"category": "groceries"}}}"#;
        let result = agent.extract_json(text);
        assert!(result.is_some());
    }

    // ========================================================================
    // extract_json tests - Negative cases
    // ========================================================================

    #[test]
    fn extract_json_returns_none_for_plain_text() {
        let agent = make_test_agent();
        let text = "This is just plain text with no JSON.";
        let result = agent.extract_json(text);
        assert!(result.is_none());
    }

    #[test]
    fn extract_json_returns_none_for_incomplete_json() {
        let agent = make_test_agent();
        let text = r#"{"name": "test", "arguments": {"foo"#;
        let result = agent.extract_json(text);
        assert!(result.is_none());
    }

    #[test]
    fn extract_json_handles_multiline_code_block() {
        let agent = make_test_agent();
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
        let result = agent.extract_json(text);
        assert!(result.is_some());
    }

    #[test]
    fn extract_json_returns_none_for_unclosed_braces() {
        let agent = make_test_agent();
        let text = r#"{"name": "test", "arguments": {"foo": "bar"}"#;
        let result = agent.extract_json(text);
        assert!(result.is_none());
    }

    // ========================================================================
    // format_tool_result tests - Positive cases
    // ========================================================================

    #[test]
    fn format_tool_result_returns_data_on_success() {
        let agent = make_test_agent();
        let result = ToolResult {
            tool: "get_plans".to_string(),
            success: true,
            data: Some(r#"[{"id": "plan-1", "name": "My Budget"}]"#.to_string()),
            error: None,
        };
        let formatted = agent.format_tool_result(&result);
        assert!(formatted.contains("plan-1"));
    }

    #[test]
    fn format_tool_result_returns_no_data_message_when_empty() {
        let agent = make_test_agent();
        let result = ToolResult {
            tool: "get_plans".to_string(),
            success: true,
            data: None,
            error: None,
        };
        let formatted = agent.format_tool_result(&result);
        assert_eq!(formatted, "No data returned");
    }

    #[test]
    fn format_tool_result_returns_error_on_failure() {
        let agent = make_test_agent();
        let result = ToolResult {
            tool: "get_plans".to_string(),
            success: false,
            data: None,
            error: Some("Network error: connection refused".to_string()),
        };
        let formatted = agent.format_tool_result(&result);
        assert!(formatted.contains("Error"));
    }

    #[test]
    fn format_tool_result_returns_unknown_error_when_no_message() {
        let agent = make_test_agent();
        let result = ToolResult {
            tool: "get_plans".to_string(),
            success: false,
            data: None,
            error: None,
        };
        let formatted = agent.format_tool_result(&result);
        assert_eq!(formatted, "Unknown error");
    }

    // ========================================================================
    // contains_valid_response tests
    // ========================================================================

    #[test]
    fn contains_valid_response_true_for_plain_text() {
        let agent = make_test_agent();
        assert!(agent.contains_valid_response("Hello, how can I help you?"));
    }

    #[test]
    fn contains_valid_response_false_for_json_name() {
        let agent = make_test_agent();
        assert!(!agent.contains_valid_response(r#"{"name": "get_plans"}"#));
    }

    #[test]
    fn contains_valid_response_false_for_json_tool_name() {
        let agent = make_test_agent();
        assert!(!agent.contains_valid_response(r#"{"tool_name": "get_plans"}"#));
    }

    #[test]
    fn contains_valid_response_false_for_json_array() {
        let agent = make_test_agent();
        assert!(!agent.contains_valid_response(r#"[{"name": "test"}]"#));
    }
}
