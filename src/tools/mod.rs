//! Tool registry for YNAB API tools

#![allow(dead_code, unused_imports, unused_variables)]

use crate::ynab::types::{
    Account, CategoryGroup, ClientError, DisplayAccount, DisplayTransaction, Month, Payee, Plan,
    ScheduledTransaction, Transaction, User,
};
use crate::ynab::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// A tool that can be called by the AI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// A tool call parsed from the AI's response.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

/// The result of executing a tool.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool: String,
    pub success: bool,
    pub data: Option<String>,
    pub error: Option<String>,
    pub suggestions: Vec<String>,
}

impl ToolResult {
    pub fn success(tool: &str, data: impl Serialize) -> Self {
        Self {
            tool: tool.to_string(),
            success: true,
            data: serde_json::to_string(&data).ok(),
            error: None,
            suggestions: vec![],
        }
    }

    pub fn error(tool: &str, e: impl std::fmt::Display) -> Self {
        Self {
            tool: tool.to_string(),
            success: false,
            data: None,
            error: Some(e.to_string()),
            suggestions: vec![],
        }
    }

    /// Create an error with a suggestion for the next action.
    pub fn error_with_suggestion(tool: &str, e: impl std::fmt::Display, suggestion: &str) -> Self {
        Self {
            tool: tool.to_string(),
            success: false,
            data: None,
            error: Some(e.to_string()),
            suggestions: vec![suggestion.to_string()],
        }
    }

    /// Create an error with multiple suggestions.
    pub fn error_with_suggestions(
        tool: &str,
        e: impl std::fmt::Display,
        suggestions: Vec<String>,
    ) -> Self {
        Self {
            tool: tool.to_string(),
            success: false,
            data: None,
            error: Some(e.to_string()),
            suggestions,
        }
    }
}

/// Tool executor function type (async).
type ToolExecutor = fn(args: &Value, client: Arc<Client>) -> ToolResult;

/// Registry of available tools.
pub struct ToolRegistry {
    tools: HashMap<String, (Tool, ToolExecutor)>,
    client: Arc<Client>,
    valid_plan_ids: RefCell<Vec<String>>,
}

impl ToolRegistry {
    pub fn new(client: Arc<Client>) -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
            client,
            valid_plan_ids: RefCell::new(vec![]),
        };
        registry.register_ynab_tools();
        registry
    }

    /// Initialize valid plan IDs by fetching from the API.
    pub fn init_plan_ids(&self) {
        if let Ok(plans) = self.client.blocking_get_plans() {
            *self.valid_plan_ids.borrow_mut() = plans.iter().map(|p| p.id.clone()).collect();
        }
    }

    /// Validate a plan_id and return an error with suggestion if invalid.
    fn validate_plan_id(&self, plan_id: &str) -> Option<ToolResult> {
        let valid_ids = self.valid_plan_ids.borrow();
        if !valid_ids.is_empty() && !valid_ids.contains(&plan_id.to_string()) {
            return Some(ToolResult::error_with_suggestion(
                "plan_id",
                format!("Invalid plan_id '{}'. Plan not found.", plan_id),
                "Call get_plans to see available plan IDs.",
            ));
        }
        None
    }

    /// Register all YNAB API tools.
    fn register_ynab_tools(&mut self) {
        let client = self.client.clone();

        // get_plans
        self.register(
            "get_plans",
            "Get all budget plans",
            Value::Null,
            |_args, client| {
                let plan_id = "";
                match client.blocking_get_plans() {
                    Ok(plans) => ToolResult::success("get_plans", &plans),
                    Err(e) => ToolResult::error("get_plans", e),
                }
            },
        );

        // get_plan
        self.register(
            "get_plan",
            "Get a specific plan by ID",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "plan_id": {"type": "string"}
                },
                "required": ["plan_id"]
            }),
            |args, client| {
                let plan_id = args["plan_id"].as_str().unwrap_or("");
                match client.blocking_get_plan(plan_id) {
                    Ok(plan) => ToolResult::success("get_plan", &plan),
                    Err(e) => ToolResult::error("get_plan", e),
                }
            },
        );

        // get_accounts
        self.register(
            "get_accounts",
            "Get all accounts for a plan. Returns account names and balances in dollars.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "plan_id": {"type": "string"}
                },
                "required": ["plan_id"]
            }),
            |args, client| {
                let plan_id = args["plan_id"].as_str().unwrap_or("");
                match client.blocking_get_accounts(plan_id) {
                    Ok(accounts) => {
                        let display_accounts: Vec<DisplayAccount> =
                            accounts.iter().map(DisplayAccount::from).collect();
                        ToolResult::success("get_accounts", &display_accounts)
                    }
                    Err(e) => ToolResult::error("get_accounts", e),
                }
            },
        );

        // get_categories
        self.register(
            "get_categories",
            "Get all categories for a plan",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "plan_id": {"type": "string"}
                },
                "required": ["plan_id"]
            }),
            |args, client| {
                let plan_id = args["plan_id"].as_str().unwrap_or("");
                match client.blocking_get_categories(plan_id) {
                    Ok(categories) => ToolResult::success("get_categories", &categories),
                    Err(e) => ToolResult::error("get_categories", e),
                }
            },
        );

        // get_payees
        self.register(
            "get_payees",
            "Get all payees for a plan. Returns list of payees with their IDs.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "plan_id": {"type": "string"}
                },
                "required": ["plan_id"]
            }),
            |args, client| {
                let plan_id = args["plan_id"].as_str().unwrap_or("");
                match client.blocking_get_payees(plan_id) {
                    Ok(payees) => ToolResult::success("get_payees", &payees),
                    Err(e) => ToolResult::error("get_payees", e),
                }
            },
        );

        // search_payee_transactions - convenience tool for finding spending at a specific payee
        self.register(
            "search_payee_transactions",
            "Search for transactions at a specific payee/store. Pass the store name as payee_search (e.g., 'Apple', 'Amazon', 'Walmart'). Returns all matching transactions sorted by date.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "plan_id": {"type": "string"},
                    "payee_search": {"type": "string", "description": "Name of payee to search for (e.g., 'Apple', 'Amazon'). Case-insensitive partial match."}
                },
                "required": ["plan_id", "payee_search"]
            }),
            |args, client| {
                let plan_id = args["plan_id"].as_str().unwrap_or("");
                let payee_search = args["payee_search"].as_str().unwrap_or("");
                match client.blocking_search_payee_transactions(plan_id, payee_search) {
                    Ok(txs) => {
                        let display_txs: Vec<DisplayTransaction> =
                            txs.iter().map(DisplayTransaction::from).collect();
                        ToolResult::success("search_payee_transactions", &display_txs)
                    }
                    Err(e) => ToolResult::error("search_payee_transactions", e),
                }
            },
        );

        // get_transactions
        self.register(
            "get_transactions",
            "Get recent transactions for a plan. Returns up to 100 transactions sorted by date (most recent first). Use since_date to filter by date.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "plan_id": {"type": "string"},
                    "limit": {"type": "integer", "description": "Max transactions to return (default 100)"},
                    "since_date": {"type": "string", "description": "Only return transactions on or after this date (YYYY-MM-DD)"}
                },
                "required": ["plan_id"]
            }),
            |args, client| {
                let plan_id = args["plan_id"].as_str().unwrap_or("");
                let since_date = args.get("since_date").and_then(|v| v.as_str());
                match client.blocking_get_transactions_paginated(plan_id, Some(100), since_date) {
                    Ok(mut txs) => {
                        // Sort by date descending (most recent first)
                        txs.sort_by(|a, b| b.date.cmp(&a.date));
                        // Limit results
                        let limit = args.get("limit").and_then(|v| v.as_i64()).map(|v| v as usize).unwrap_or(100);
                        txs.truncate(limit);
                        let display_txs: Vec<DisplayTransaction> =
                            txs.iter().map(DisplayTransaction::from).collect();
                        ToolResult::success("get_transactions", &display_txs)
                    }
                    Err(e) => ToolResult::error("get_transactions", e),
                }
            },
        );

        // get_transactions_by_month
        self.register(
            "get_transactions_by_month",
            "Get transactions for a specific month. Use YYYY-MM-DD format for the month parameter (e.g., '2026-04-01' for April 2026).",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "plan_id": {"type": "string"},
                    "month": {"type": "string", "description": "Month in YYYY-MM-DD format (e.g., '2026-04-01')"}
                },
                "required": ["plan_id", "month"]
            }),
            |args, client| {
                let plan_id = args["plan_id"].as_str().unwrap_or("");
                let month = args["month"].as_str().unwrap_or("");
                match client.blocking_get_transactions_by_month(plan_id, month) {
                    Ok(txs) => {
                        let display_txs: Vec<DisplayTransaction> =
                            txs.iter().map(DisplayTransaction::from).collect();
                        ToolResult::success("get_transactions_by_month", &display_txs)
                    }
                    Err(e) => ToolResult::error("get_transactions_by_month", e),
                }
            },
        );

        // get_month
        self.register(
            "get_month",
            "Get budget summary for a specific month",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "plan_id": {"type": "string"},
                    "month": {"type": "string", "description": "Month in YYYY-MM format (e.g., '2026-04')"}
                },
                "required": ["plan_id", "month"]
            }),
            |args, client| {
                let plan_id = args["plan_id"].as_str().unwrap_or("");
                let month = args["month"].as_str().unwrap_or("");
                match client.blocking_get_month(plan_id, month) {
                    Ok(month_data) => ToolResult::success("get_month", &month_data),
                    Err(e) => ToolResult::error("get_month", e),
                }
            },
        );

        // get_scheduled_transactions
        self.register(
            "get_scheduled_transactions",
            "Get scheduled transactions for a plan",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "plan_id": {"type": "string"}
                },
                "required": ["plan_id"]
            }),
            |args, client| {
                let plan_id = args["plan_id"].as_str().unwrap_or("");
                match client.blocking_get_scheduled_transactions(plan_id) {
                    Ok(scheduled) => ToolResult::success("get_scheduled_transactions", &scheduled),
                    Err(e) => ToolResult::error("get_scheduled_transactions", e),
                }
            },
        );
    }

    /// Register a tool with the registry.
    fn register(
        &mut self,
        name: &str,
        description: &str,
        parameters: Value,
        executor: ToolExecutor,
    ) {
        self.tools.insert(
            name.to_string(),
            (
                Tool {
                    name: name.to_string(),
                    description: description.to_string(),
                    parameters,
                },
                executor,
            ),
        );
    }

    /// Tools that require a plan_id parameter.
    const PLAN_ID_REQUIRED_TOOLS: &'static [&'static str] = &[
        "get_plan",
        "get_accounts",
        "get_categories",
        "get_payees",
        "search_payee_transactions",
        "get_transactions",
        "get_transactions_by_month",
        "get_month",
        "get_scheduled_transactions",
    ];

    /// Execute a tool by name with the given arguments (synchronous).
    pub fn execute(&self, name: &str, arguments: Value) -> ToolResult {
        // Check if tool exists
        let Some((tool, executor)) = self.tools.get(name) else {
            return ToolResult::error_with_suggestion(
                name,
                format!(
                    "Unknown tool: {}. Use get_plans to see available tools.",
                    name
                ),
                "Call get_plans to see all available tools.",
            );
        };

        // Validate plan_id for tools that require it
        if Self::PLAN_ID_REQUIRED_TOOLS.contains(&name) {
            if let Some(plan_id) = arguments.get("plan_id").and_then(|v| v.as_str()) {
                let valid_ids = self.valid_plan_ids.borrow();
                if !valid_ids.is_empty() && !valid_ids.contains(&plan_id.to_string()) {
                    return ToolResult::error_with_suggestion(
                        name,
                        format!(
                            "Invalid plan_id '{}'. This plan does not exist or you do not have access.",
                            plan_id
                        ),
                        "Call get_plans to see available plan IDs.",
                    );
                }
            } else if tool.parameters != Value::Null {
                // Parameter is required but missing
                return ToolResult::error_with_suggestion(
                    name,
                    "Missing required parameter: plan_id",
                    "Provide a valid plan_id from get_plans.",
                );
            }
        }

        // Execute the tool
        let result = executor(&arguments, self.client.clone());

        // After get_plans succeeds, cache the plan IDs
        if name == "get_plans" && result.success {
            if let Some(ref data) = result.data {
                if let Ok(plans) = serde_json::from_str::<Vec<Plan>>(data) {
                    *self.valid_plan_ids.borrow_mut() =
                        plans.iter().map(|p| p.id.clone()).collect();
                }
            }
        }

        result
    }

    /// Get all tool definitions for the AI.
    pub fn get_definitions(&self) -> Vec<Tool> {
        self.tools.values().map(|(t, _)| t.clone()).collect()
    }
}

// Extension trait for blocking versions
impl Client {
    pub fn blocking_get_user(&self) -> Result<User, ClientError> {
        self.runtime_block_on(self.get_user())
    }

    pub fn blocking_get_plans(&self) -> Result<Vec<Plan>, ClientError> {
        self.runtime_block_on(self.get_plans())
    }

    pub fn blocking_get_plan(&self, plan_id: &str) -> Result<Plan, ClientError> {
        self.runtime_block_on(self.get_plan(plan_id))
    }

    pub fn blocking_get_accounts(&self, plan_id: &str) -> Result<Vec<Account>, ClientError> {
        self.runtime_block_on(self.get_accounts(plan_id))
    }

    pub fn blocking_get_categories(
        &self,
        plan_id: &str,
    ) -> Result<Vec<CategoryGroup>, ClientError> {
        self.runtime_block_on(self.get_categories(plan_id))
    }

    pub fn blocking_get_payees(&self, plan_id: &str) -> Result<Vec<Payee>, ClientError> {
        self.runtime_block_on(self.get_payees(plan_id))
    }

    pub fn blocking_get_transactions(
        &self,
        plan_id: &str,
    ) -> Result<Vec<Transaction>, ClientError> {
        self.runtime_block_on(self.get_transactions(plan_id))
    }

    pub fn blocking_get_transactions_paginated(
        &self,
        plan_id: &str,
        limit: Option<i32>,
        since_date: Option<&str>,
    ) -> Result<Vec<Transaction>, ClientError> {
        self.runtime_block_on(self.get_transactions_paginated(plan_id, limit, since_date))
    }

    pub fn blocking_search_payee_transactions(
        &self,
        plan_id: &str,
        payee_search: &str,
    ) -> Result<Vec<Transaction>, ClientError> {
        self.runtime_block_on(self.search_payee_transactions(plan_id, payee_search))
    }

    pub fn blocking_get_transactions_by_payee(
        &self,
        plan_id: &str,
        payee_id: &str,
    ) -> Result<Vec<Transaction>, ClientError> {
        self.runtime_block_on(self.get_transactions_by_payee(plan_id, payee_id))
    }

    pub fn blocking_get_transactions_by_month(
        &self,
        plan_id: &str,
        month: &str,
    ) -> Result<Vec<Transaction>, ClientError> {
        self.runtime_block_on(self.get_transactions_by_month(plan_id, month))
    }

    pub fn blocking_get_month(&self, plan_id: &str, month: &str) -> Result<Month, ClientError> {
        self.runtime_block_on(self.get_month(plan_id, month))
    }

    pub fn blocking_get_scheduled_transactions(
        &self,
        plan_id: &str,
    ) -> Result<Vec<ScheduledTransaction>, ClientError> {
        self.runtime_block_on(self.get_scheduled_transactions(plan_id))
    }

    fn runtime_block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        // Use block_in_place to run the future in a blocking-safe manner
        // This works because we're in a multi-thread tokio runtime
        tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
    }
}
