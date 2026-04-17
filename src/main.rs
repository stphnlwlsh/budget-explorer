//! Budget Explorer - Agentic CLI for YNAB budget planning.
//!
//! Run with: cargo run -- [command] [options]

#![allow(dead_code, unused_imports, unused_variables)]

mod ai;
mod config;
mod profile;
mod tools;
mod ynab;

use ai::{build_system_prompt, Agent, OllamaProvider};
use clap::{Parser, Subcommand};
use profile::UserProfile;
use std::io::{self, Write};
use std::sync::Arc;
use tools::ToolRegistry;
use ynab::Client;

/// Load environment from .env.local if present (not auto-loaded by cargo)
fn load_env() {
    dotenvy::dotenv().ok();
    if let Ok(content) = std::fs::read_to_string(".env.local") {
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                if let Some((key, value)) = line.split_once('=') {
                    std::env::set_var(key.trim(), value.trim());
                }
            }
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    load_env();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Chat { debug, query }) => {
            if let Err(e) = chat(debug, query).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Onboarding) => {
            if let Err(e) = run_onboarding() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::UpdateProfile) => {
            if let Err(e) = update_profile() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::ListPlans) => {
            if let Err(e) = list_plans().await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::ShowConfig) => {
            show_config();
        }
        None => {
            println!("Budget Explorer - Use 'cargo run -- chat' to start\n");
            println!("Run 'cargo run -- --help' for all commands");
        }
    }
}

#[derive(Parser)]
#[command(name = "budget-explorer")]
#[command(about = "Agentic CLI for YNAB budget planning with AI-powered insights")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start AI chat with your budget data
    Chat {
        /// Enable debug output
        #[arg(long, default_value = "false")]
        debug: bool,
        /// Run a single query and exit
        #[arg(short, long)]
        query: Option<String>,
    },
    /// Run first-time setup wizard
    Onboarding,
    /// Update your profile preferences
    UpdateProfile,
    /// List all budget plans
    ListPlans,
    /// Show current configuration
    ShowConfig,
}

// ============================================================================
// Chat Command
// ============================================================================

async fn chat(debug: bool, initial_query: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Get YNAB token
    let token = std::env::var("YNAB_ACCESS_TOKEN")
        .map_err(|_| "YNAB_ACCESS_TOKEN not set. Add it to .env or .env.local")?;

    // Create client
    let client = Arc::new(Client::new(&token));

    // Get Ollama config
    let base_url = std::env::var("OLLAMA_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    let model = std::env::var("OLLAMA_MODEL")
        .map_err(|_| "OLLAMA_MODEL not set. Add it to .env or .env.local")?;

    let llm = Arc::new(OllamaProvider::new(&base_url, &model));
    let registry = Arc::new(ToolRegistry::new(Arc::clone(&client)));

    // Convert tool definitions to Ollama format
    let ollama_tools = registry
        .get_definitions()
        .into_iter()
        .map(|tool| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters
                }
            })
        })
        .collect();

    let mut agent = Agent::new(registry.clone(), llm).with_tools(ollama_tools);

    // Load profile
    let profile = UserProfile::load().ok().flatten();

    // Build system prompt (for text fallback parsing)
    let tools_json = serde_json::to_string(&registry.get_definitions())
        .map_err(|e| format!("Failed to serialize tools: {}", e))?;
    let system_prompt = build_system_prompt(profile.as_ref(), &tools_json);

    if debug {
        println!("╔══════════════════════════════════════════════════════════════════╗");
        println!("║                         DEBUG MODE                               ║");
        println!("╚══════════════════════════════════════════════════════════════════╝");
        println!("\n--- SYSTEM PROMPT ---");
        println!("{}", system_prompt);
        println!("\n--- AVAILABLE TOOLS ({}) ---", registry.get_definitions().len());
    }

    // If initial query, run it and exit
    if let Some(query) = initial_query {
        run_query(&mut agent, &system_prompt, &query, debug).await;
        return Ok(());
    }

    // Interactive mode
    println!("Budget Explorer Chat (type 'exit' to quit)\n");
    loop {
        print!("You: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() || input.to_lowercase() == "exit" {
            break;
        }

        run_query(&mut agent, &system_prompt, input, debug).await;
    }

    Ok(())
}

async fn run_query(agent: &mut Agent, system_prompt: &str, query: &str, debug: bool) {
    if debug {
        println!("\n--- USER INPUT ---\n{}", query);
    }

    // Show thinking indicator on stderr
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    let handle = std::thread::spawn(move || {
        let dots = ["   ", ".  ", ".. ", "..."];
        let mut i = 0;
        while running_clone.load(Ordering::Relaxed) {
            eprint!("\rThinking{}", dots[i % 4]);
            std::thread::sleep(std::time::Duration::from_millis(300));
            i += 1;
        }
        eprint!("\r          \r");
    });

    let result = agent.run(query, system_prompt).await;
    running.store(false, Ordering::Relaxed);
    // Wait for the thinking thread to finish and clear its output
    let _ = handle.join();

    match result {
        Ok(response) => {
            if debug {
                println!("\n--- LLM RESPONSE ---\n{}\n", response);
            }
            println!("\nBudget Explorer: {}\n", response);
        }
        Err(e) => {
            println!("\nError: {}\n", e);
        }
    }
}

// ============================================================================
// Onboarding Command
// ============================================================================

fn run_onboarding() -> Result<(), Box<dyn std::error::Error>> {
    let profile = profile::run()?;
    println!("Profile created: {:?}", profile.advice_style);
    Ok(())
}

// ============================================================================
// Update Profile Command
// ============================================================================

fn update_profile() -> Result<(), Box<dyn std::error::Error>> {
    let mut profile = UserProfile::load()?
        .ok_or("No profile found. Run 'budget-explorer onboarding' first")?;
    profile::update(&mut profile)?;
    Ok(())
}

// ============================================================================
// List Plans Command
// ============================================================================

async fn list_plans() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("YNAB_ACCESS_TOKEN")
        .map_err(|_| "YNAB_ACCESS_TOKEN not set")?;
    let client = Client::new(&token);

    let plans = client.get_plans().await?;

    println!("\nYour Budget Plans:\n");
    for plan in &plans {
        println!("  {} - {}", plan.name, plan.id);
    }
    println!();

    Ok(())
}

// ============================================================================
// Show Config Command
// ============================================================================

fn show_config() {
    println!("\n=== Budget Explorer Configuration ===\n");

    // YNAB Token
    match std::env::var("YNAB_ACCESS_TOKEN") {
        Ok(_) => println!("✓ YNAB_ACCESS_TOKEN set"),
        Err(_) => println!("✗ YNAB_ACCESS_TOKEN not set"),
    }

    // Ollama
    let ollama_url = std::env::var("OLLAMA_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    let ollama_model = std::env::var("OLLAMA_MODEL")
        .unwrap_or_else(|_| "(not set)".to_string());
    println!("✓ OLLAMA_BASE_URL: {}", ollama_url);
    println!("✓ OLLAMA_MODEL: {}", ollama_model);

    // Profile
    match UserProfile::load() {
        Ok(Some(p)) => {
            println!("✓ Profile loaded:");
            println!("  Goals: {}", p.goals.join(", "));
            println!("  Style: {:?}", p.advice_style);
            println!("  Tone: {:?}", p.tone);
        }
        Ok(None) => println!("✗ No profile found (run 'onboarding')"),
        Err(e) => println!("✗ Error loading profile: {}", e),
    }

    println!();
}
