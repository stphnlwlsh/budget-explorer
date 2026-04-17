# Budget Explorer Tests

This document describes the test structure for the budget-explorer project.

## Test Organization

Tests are organized as inline `#[cfg(test)]` modules within the source files:

### Priority 1: YNAB Types (`src/ynab/types.rs`)
Tests for pure helper functions:
- `milliunits_to_dollars()` - converts milliunits to f64 (5 tests)
- `format_milliunits()` - formats as "$123.45" (5 tests)
- `aggregate_by_payee()` - groups transactions by payee (5 tests)
- `calculate_category_spending()` - sums category spending (4 tests)
- `filter_transactions_by_date()` - filters by date range (4 tests)
- `find_category()` - case-insensitive category search (6 tests)
- `find_payee()` - case-insensitive payee search (5 tests)

**Total: 34 tests**

### Priority 2: Tool Registry (`src/tools/mod.rs`)
Tests for tool structures and registry:
- `Tool` serialization/deserialization (2 tests)
- `ToolCall` deserialization (2 tests)
- `ToolResult` success and error handling (4 tests)
- `ToolRegistry` tool registration (4 tests)

**Total: 12 tests**

### Priority 3: AI Agent (`src/ai/agent.rs`)
Tests for JSON parsing and tool result formatting:
- `extract_json()` - parse JSON from various formats (8 tests)
- `format_tool_result()` - format success/error results (4 tests)

**Total: 12 tests**

### Priority 4: Profile (`src/profile.rs`)
Tests for profile serialization:
- `UserProfile` serialization/deserialization (4 tests)
- `default_profile()` creation (2 tests)
- `AdviceStyle`/`Tone` serialization (4 tests)
- persistence path validation (2 tests)

**Total: 12 tests**

## Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific module
cargo test --lib ynab::types::tests
cargo test --lib tools::tests
cargo test --lib ai::agent::tests
cargo test --lib profile::tests
```

## Test Conventions

1. **AAA Pattern**: Arrange → Act → Assert
2. **Naming**: snake_case with descriptive names (`test_function_expected_behavior`)
3. **Positive + Negative**: Both success and failure cases are tested
4. **No External Dependencies**: All tests are deterministic (no network, no real API calls)
5. **Independent**: Tests don't depend on each other or shared state

## Dependencies

```toml
[dev-dependencies]
wiremock = "0.6"  # HTTP mocking for async tests
tempfile = "3"    # Temp directories for file I/O tests
```

## Notes

- The Agent tests require creating a mock LLM provider because the Agent depends on a `dyn LLMProvider` trait
- The ToolRegistry tests don't run async execution (which would require tokio runtime) - they test the definitions instead
- Profile tests don't write to actual files due to environment differences, but verify serialization is correct