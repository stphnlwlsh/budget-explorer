//! GAME Framework - Goals and Environment
//!
//! G - Goals: What the agent is trying to achieve
//! E - Environment: Abstract interface to external systems

use crate::profile::UserProfile;
use crate::ynab::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// G - Goals: What the agent is trying to achieve
// ============================================================================

/// A goal for the agent to pursue.
///
/// Goals have priority: higher = more important.
/// - Agent goals (hardcoded): 80-100 (always present)
/// - User goals (from onboarding): 1-20 (shape responses, not behavior)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    /// Unique identifier for the goal.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Priority (1-100). Agent goals: 80-100. User goals: 1-20.
    pub priority: u8,
}

impl Goal {
    /// Create a new goal.
    pub fn new(name: &str, description: &str, priority: u8) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            priority,
        }
    }
}

/// Standard agent goals - always present for budget-explorer.
pub fn standard_goals() -> Vec<Goal> {
    vec![
        Goal::new(
            "answer_budget_questions",
            "Answer user questions about their budget using YNAB API tools. Get actual data, never estimate.",
            95,
        ),
        Goal::new(
            "explain_patterns",
            "Explain WHY spending patterns exist. Use the data to provide reasoning, not just numbers.",
            90,
        ),
        Goal::new(
            "personalize_responses",
            "Tailor responses to user's goals, concerns, and preferred communication style.",
            85,
        ),
        Goal::new(
            "ensure_accuracy",
            "Always use tools to get real data. Never make up numbers or pretend to know.",
            100,
        ),
    ]
}

/// Extend agent goals with user goals from onboarding (lower priority).
pub fn merge_goals(agent_goals: Vec<Goal>, user_profile: Option<&UserProfile>) -> Vec<Goal> {
    let mut goals = agent_goals;

    if let Some(profile) = user_profile {
        // Add user goals with low priority (1-20)
        for (i, goal_text) in profile.goals.iter().enumerate() {
            goals.push(Goal::new(
                &format!("user_goal_{}", i),
                goal_text,
                10, // Low priority - shapes how, not what
            ));
        }
    }

    // Sort by priority descending
    goals.sort_by(|a, b| b.priority.cmp(&a.priority));
    goals
}

// ============================================================================
// E - Environment: Abstract interface to external world
// ============================================================================

/// Environment trait - abstracts how the agent interacts with external systems.
///
/// This separates "what tools do" from "how they do it" (GAME pattern).
/// Allows swapping implementations (real YNAB API, mocks, test harnesses).
pub trait Environment: Send + Sync {
    /// Get the name of this environment.
    fn name(&self) -> &str;

    /// Get a description for the agent.
    fn description(&self) -> &str;
}

/// YNAB Environment - connects to the YNAB API.
pub struct YnabEnvironment {
    client: Arc<Client>,
}

impl YnabEnvironment {
    /// Create a new YNAB environment.
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Get the underlying client.
    pub fn client(&self) -> &Arc<Client> {
        &self.client
    }
}

impl Environment for YnabEnvironment {
    fn name(&self) -> &str {
        "YNAB API"
    }

    fn description(&self) -> &str {
        "Connected to YNAB budget planning API"
    }
}
