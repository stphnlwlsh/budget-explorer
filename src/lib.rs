//! Budget Explorer Library

#![allow(dead_code, unused_imports, unused_variables)]

pub mod ai;
pub mod config;
pub mod profile;
pub mod tools;
pub mod ynab;

// Re-exports
pub use ai::{LLMProvider, Message};
pub use config::{Config, ConfigError};
pub use profile::UserProfile;
pub use tools::{ToolRegistry, ToolResult};
pub use ynab::Client;
