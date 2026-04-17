# Budget Explorer

An **agentic CLI** that brings AI-powered budget insights directly to your terminal. Ask questions in plain English like *"How much did I spend on groceries last month?"* or *"Which categories am I overspending in?"* — and get instant, personalized answers from your YNAB (You Need A Budget) data.

## What It Does

Budget Explorer connects to your YNAB account and provides two ways to explore your finances:

### 1. Interactive AI Chat

```bash
cargo run -- chat
```

Start a conversation with an AI that has access to your complete budget data. The agent uses **tool-calling** to fetch exactly the information you need:

- *"How much did I spend at Amazon this year?"*
- *"Show my top 5 spending categories last month"*
- *"What are my upcoming bills?"*
- *"Am I on track with my savings goal?"*

The AI personalizes responses based on your profile preferences — choosing a detailed breakdown, concise summary, or action-oriented advice.

### 2. CLI Commands

For quick lookups, use these direct commands:

| Command | Description |
|---------|-------------|
| `cargo run -- query --list-plans` | List all your budget plans |
| `cargo run -- query --list-categories` | Show all budget categories |
| `cargo run -- query --category "Food"` | Show spending by payee for a category |
| `cargo run -- set-default-plan <id>` | Set default budget plan |
| `cargo run -- unused-payees` | Find payees with no transactions |
| `cargo run -- show-config` | Display current configuration |

## How It Works (Architecture)

```
┌─────────────────────────────────────────────────────────────┐
│                     Budget Explorer CLI                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────┐    ┌──────────────┐    ┌─────────────────┐ │
│  │   CLI    │───▶│    Agent     │───▶│  Tool Registry  │ │
│  │ (clap)  │    │ (LLM + Tools) │    │  (15 YNAB API)  │ │
│  └──────────┘    └──────────────┘    └─────────────────┘ │
│                           │                    │           │
│                           ▼                    ▼           │
│                    ┌──────────────┐    ┌─────────────┐ │
│                    │  Ollama LLM   │    │  YNAB API  │ │
│                    │ (Gemma/3.2)  │    │  Client    │ │
│                    └──────────────┘    └─────────────┘ │
│                                            │           │
│                                            ▼           │
│                                    ┌──────────────────┐ │
│                                    │  User Profile    │ │
│                                    │  (Preferences)  │ │
│                                    └──────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### The Agent System

The AI agent uses **tool-calling** (function calling) to interact with your budget:

1. **System Prompt** — Built using the **GAIL framework** to personalize responses:
   - **G**oals: What you want to achieve (from your profile)
   - **A**ctions: What the AI can do (15 YNAB API tools)
   - **I**nformation: Your budget data (transactions, categories, accounts)
   - **L**anguage: Your preferred communication style

2. **Tool Registry** — 15 self-describing tools that the AI can call:
   - `get_transactions` — Recent transactions with amounts
   - `get_categories` — Budget categories by group
   - `get_payees` — All payees (stores, utilities)
   - `get_months` — Monthly summaries
   - `get_accounts` — Checking, savings, credit cards
   - `get_scheduled_transactions` — Upcoming bills
   - And more...

3. **User Profile** — Stored locally, defines:
   - **Goals**: Financial objectives (e.g., "save for vacation")
   - **Concerns**: Areas to address (e.g., "reduce dining out")
   - **Advice Style**: Detailed, Summary, or Action Items
   - **Tone**: Friendly, Direct, or Professional

### Storage & Security

- **Config**: `~/Library/Application Support/budget-explorer/config.json`
- **Profile**: `~/Library/Application Support/budget-explorer/profile.json`
- **API Token**: Stored in macOS Keychain (preferred) or `.env` file

## Benefits for You

| Benefit | How It's Delivered |
|---------|-------------------|
| **Natural language queries** | Ask *"How much did I spend?"* instead of navigating menus |
| **Personalized advice** | AI adapts to your goals, concerns, and communication style |
| **Privacy-first** | All data stays on your machine (local Ollama LLM) |
| **Fast insights** | CLI commands for instant answers without AI overhead |
| **Full budget access** | Complete read access to all YNAB data via API |

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) installed
- [Ollama](https://ollama.com/) running locally
- [YNAB](https://youneedabudget.com/) account with your budget set up

### Setup

1. **Get your YNAB API token**:
   - Go to YNAB Settings → Developer
   - Create a new access token

2. **Store the token** (choose one):

   **Option A: macOS Keychain** (recommended)
   ```bash
   security add-generic-password -s YNAB_ACCESS_TOKEN -a <your-username> -w <your-token>
   ```

   **Option B: Environment file**
   ```bash
   echo "YNAB_ACCESS_TOKEN=your-token-here" > .env
   ```

3. **Configure Ollama**:
   ```bash
   export OLLAMA_BASE_URL=http://localhost:11434
   export OLLAMA_MODEL=llama3.2  # or gemma:2b
   ```

4. **Run onboarding**:
   ```bash
   cargo run -- onboarding
   ```

5. **Start chatting**:
   ```bash
   cargo run -- chat
   ```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Language | Rust 2021 |
| CLI | [clap](https://docs.rs/clap/) |
| HTTP | [reqwest](https://docs.rs/reqwest/) |
| Async | [tokio](https://tokio.rs/) |
| LLM | [Ollama](https://ollama.com/) (Gemma, Llama 3.2) |
| Serialization | [serde](https://serde.rs/) |

## Troubleshooting

**"Failed to initialize Ollama"**
- Make sure Ollama is running: `ollama serve`
- Check `OLLAMA_BASE_URL` and `OLLAMA_MODEL` are set

**"YNAB_ACCESS_TOKEN required"**
- Run onboarding: `cargo run -- onboarding`
- Or set the token in `.env` / Keychain

**"No profile found"**
- First run triggers onboarding automatically
- Or run: `cargo run -- onboarding`

## License

MIT