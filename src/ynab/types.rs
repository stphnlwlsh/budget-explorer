//! YNAB API types

#![allow(dead_code, unused_imports, unused_variables)]

use serde::de::{self, Deserializer, Visitor};
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

// ============================================================================
// Client (async)
// ============================================================================

pub struct Client {
    access_token: String,
    http: reqwest::Client,
    base_url: String,
}

impl Client {
    pub fn new(access_token: &str) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            access_token: access_token.to_string(),
            http,
            base_url: "https://api.ynab.com/v1".to_string(),
        }
    }

    async fn get<T: for<'de> serde::Deserialize<'de>>(&self, endpoint: &str) -> Result<T, ClientError> {
        let url = format!("{}{}", self.base_url, endpoint);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .map_err(|e| ClientError::Network(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ClientError::Api { status, body });
        }
        resp.json::<T>()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))
    }

    // User
    pub async fn get_user(&self) -> Result<User, ClientError> {
        let resp: UserResponse = self.get("/user").await?;
        Ok(resp.data.user)
    }

    // Plans
    pub async fn get_plans(&self) -> Result<Vec<Plan>, ClientError> {
        let resp: PlansResponse = self.get("/plans").await?;
        Ok(resp.data.plans)
    }

    pub async fn get_plan(&self, plan_id: &str) -> Result<Plan, ClientError> {
        let resp: PlanResponse = self.get(&format!("/plans/{}", plan_id)).await?;
        Ok(resp.data.plan)
    }

    pub async fn get_plan_settings(&self, plan_id: &str) -> Result<PlanSettings, ClientError> {
        let resp: PlanSettingsResponse = self.get(&format!("/plans/{}/settings", plan_id)).await?;
        Ok(resp.data.settings)
    }

    // Accounts
    pub async fn get_accounts(&self, plan_id: &str) -> Result<Vec<Account>, ClientError> {
        let resp: AccountsResponse = self.get(&format!("/plans/{}/accounts", plan_id)).await?;
        Ok(resp.data.accounts)
    }

    pub async fn get_account(&self, plan_id: &str, account_id: &str) -> Result<Account, ClientError> {
        let resp: AccountResponse =
            self.get(&format!("/plans/{}/accounts/{}", plan_id, account_id)).await?;
        Ok(resp.data.account)
    }

    // Categories
    pub async fn get_categories(&self, plan_id: &str) -> Result<Vec<CategoryGroup>, ClientError> {
        let resp: CategoryGroupsResponse = self.get(&format!("/plans/{}/categories", plan_id)).await?;
        Ok(resp.data.category_groups)
    }

    pub async fn get_category(&self, plan_id: &str, category_id: &str) -> Result<Category, ClientError> {
        let resp: CategoryResponse =
            self.get(&format!("/plans/{}/categories/{}", plan_id, category_id)).await?;
        Ok(resp.data.category)
    }

    pub async fn get_month_category(
        &self,
        plan_id: &str,
        month: &str,
        category_id: &str,
    ) -> Result<MonthCategory, ClientError> {
        let resp: MonthCategoryResponse = self.get(&format!(
            "/plans/{}/months/{}/categories/{}",
            plan_id, month, category_id
        )).await?;
        Ok(resp.data.month_category)
    }

    // Payees
    pub async fn get_payees(&self, plan_id: &str) -> Result<Vec<Payee>, ClientError> {
        let resp: PayeesResponse = self.get(&format!("/plans/{}/payees", plan_id)).await?;
        Ok(resp.data.payees)
    }

    pub async fn get_payee(&self, plan_id: &str, payee_id: &str) -> Result<Payee, ClientError> {
        let resp: PayeeResponse = self.get(&format!("/plans/{}/payees/{}", plan_id, payee_id)).await?;
        Ok(resp.data.payee)
    }

    // Months
    pub async fn get_months(&self, plan_id: &str) -> Result<Vec<Month>, ClientError> {
        let resp: MonthsResponse = self.get(&format!("/plans/{}/months", plan_id)).await?;
        Ok(resp.data.months)
    }

    pub async fn get_month(&self, plan_id: &str, month: &str) -> Result<Month, ClientError> {
        let resp: MonthResponse = self.get(&format!("/plans/{}/months/{}", plan_id, month)).await?;
        Ok(resp.data.month)
    }

    // Transactions
    pub async fn get_transactions(&self, plan_id: &str) -> Result<Vec<Transaction>, ClientError> {
        let resp: TransactionsResponse = self.get(&format!("/plans/{}/transactions", plan_id)).await?;
        Ok(resp.data.transactions)
    }

    pub async fn get_transactions_paginated(
        &self,
        plan_id: &str,
        limit: Option<i32>,
        since_date: Option<&str>,
    ) -> Result<Vec<Transaction>, ClientError> {
        let mut endpoint = format!("/plans/{}/transactions", plan_id);
        let mut params = Vec::new();
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(sd) = since_date {
            params.push(format!("since_date={}", sd));
        }
        if !params.is_empty() {
            endpoint = format!("{}?{}", endpoint, params.join("&"));
        }
        let resp: TransactionsResponse = self.get(&endpoint).await?;
        Ok(resp.data.transactions)
    }

    pub async fn get_transaction(
        &self,
        plan_id: &str,
        transaction_id: &str,
    ) -> Result<Transaction, ClientError> {
        let resp: TransactionResponse = self.get(&format!(
            "/plans/{}/transactions/{}",
            plan_id, transaction_id
        )).await?;
        Ok(resp.data.transaction)
    }

    pub async fn get_transactions_by_month(
        &self,
        plan_id: &str,
        month: &str,
    ) -> Result<Vec<Transaction>, ClientError> {
        let resp: TransactionsResponse =
            self.get(&format!("/plans/{}/months/{}/transactions", plan_id, month)).await?;
        Ok(resp.data.transactions)
    }

    pub async fn get_transactions_by_account(
        &self,
        plan_id: &str,
        account_id: &str,
    ) -> Result<Vec<Transaction>, ClientError> {
        let resp: TransactionsResponse = self.get(&format!(
            "/plans/{}/accounts/{}/transactions",
            plan_id, account_id
        )).await?;
        Ok(resp.data.transactions)
    }

    pub async fn get_transactions_by_category(
        &self,
        plan_id: &str,
        category_id: &str,
    ) -> Result<Vec<Transaction>, ClientError> {
        let resp: TransactionsResponse = self.get(&format!(
            "/plans/{}/categories/{}/transactions",
            plan_id, category_id
        )).await?;
        Ok(resp.data.transactions)
    }

    /// Search for payees by name and return their transactions.
    /// This is a convenience method that combines get_payees and get_transactions_by_payee.
    pub async fn search_payee_transactions(
        &self,
        plan_id: &str,
        payee_search: &str,
    ) -> Result<Vec<Transaction>, ClientError> {
        // First get all payees
        let payees: Vec<Payee> = self.get_payees(plan_id).await?;
        
        // Find payee IDs that contain the search term (case-insensitive)
        let matching_payee_ids: Vec<String> = payees
            .into_iter()
            .filter(|p| !p.deleted && p.name.to_lowercase().contains(&payee_search.to_lowercase()))
            .map(|p| p.id)
            .collect();
        
        // Get transactions for all matching payees
        let mut all_transactions = Vec::new();
        for payee_id in &matching_payee_ids {
            let transactions = self.get_transactions_by_payee(plan_id, payee_id).await?;
            all_transactions.extend(transactions);
        }
        
        // Sort by date descending
        all_transactions.sort_by(|a, b| b.date.cmp(&a.date));
        
        Ok(all_transactions)
    }

    pub async fn get_transactions_by_payee(
        &self,
        plan_id: &str,
        payee_id: &str,
    ) -> Result<Vec<Transaction>, ClientError> {
        let resp: TransactionsResponse = self.get(&format!(
            "/plans/{}/payees/{}/transactions",
            plan_id, payee_id
        )).await?;
        Ok(resp.data.transactions)
    }

    // Scheduled Transactions
    pub async fn get_scheduled_transactions(
        &self,
        plan_id: &str,
    ) -> Result<Vec<ScheduledTransaction>, ClientError> {
        let resp: ScheduledTransactionsResponse =
            self.get(&format!("/plans/{}/scheduled_transactions", plan_id)).await?;
        Ok(resp.data.scheduled_transactions)
    }

    pub async fn get_scheduled_transaction(
        &self,
        plan_id: &str,
        scheduled_transaction_id: &str,
    ) -> Result<ScheduledTransaction, ClientError> {
        let resp: ScheduledTransactionResponse = self.get(&format!(
            "/plans/{}/scheduled_transactions/{}",
            plan_id, scheduled_transaction_id
        )).await?;
        Ok(resp.data.scheduled_transaction)
    }

    // Money Movements
    pub async fn get_money_movements(&self, plan_id: &str) -> Result<Vec<MoneyMovement>, ClientError> {
        let resp: MoneyMovementsResponse =
            self.get(&format!("/plans/{}/money_movements", plan_id)).await?;
        Ok(resp.data.money_movements)
    }

    pub async fn get_month_money_movements(
        &self,
        plan_id: &str,
        month: &str,
    ) -> Result<Vec<MoneyMovement>, ClientError> {
        let resp: MoneyMovementsResponse = self.get(&format!(
            "/plans/{}/months/{}/money_movements",
            plan_id, month
        )).await?;
        Ok(resp.data.money_movements)
    }
}

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug)]
pub enum ClientError {
    Network(String),
    Api {
        status: reqwest::StatusCode,
        body: String,
    },
    Parse(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::Network(e) => write!(f, "Network error: {}", e),
            ClientError::Api { status, body } => write!(f, "API error {}: {}", status, body),
            ClientError::Parse(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for ClientError {}

// ============================================================================
// User
// ============================================================================

#[derive(Deserialize)]
pub struct UserResponse {
    pub data: UserData,
}

#[derive(Deserialize)]
pub struct UserData {
    pub user: User,
}

#[derive(Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
}

// ============================================================================
// Plans
// ============================================================================

#[derive(Deserialize)]
pub struct PlansResponse {
    pub data: PlansData,
}

#[derive(Deserialize)]
pub struct PlansData {
    pub plans: Vec<Plan>,
}

#[derive(Deserialize, Serialize)]
pub struct Plan {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct PlanResponse {
    pub data: PlanData,
}

#[derive(Deserialize)]
pub struct PlanData {
    pub plan: Plan,
}

#[derive(Deserialize)]
pub struct PlanSettingsResponse {
    pub data: PlanSettingsData,
}

#[derive(Deserialize)]
pub struct PlanSettingsData {
    pub settings: PlanSettings,
}

#[derive(Deserialize)]
pub struct PlanSettings {
    pub currency_code: String,
    pub date_format: String,
}

// ============================================================================
// Accounts
// ============================================================================

#[derive(Deserialize)]
pub struct AccountsResponse {
    pub data: AccountsData,
}

#[derive(Deserialize)]
pub struct AccountsData {
    pub accounts: Vec<Account>,
}

#[derive(Deserialize, Serialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub on_budget: bool,
    pub closed: bool,
    pub note: Option<String>,
    pub balance: i64,
    pub cleared_balance: i64,
    pub uncleared_balance: i64,
    pub transfer_payee_id: String,
    pub deleted: bool,
}

/// Display-friendly account with formatted balances (for LLM).
#[derive(Serialize)]
pub struct DisplayAccount {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub on_budget: bool,
    pub balance: String,
    pub cleared_balance: String,
    pub uncleared_balance: String,
}

impl From<&Account> for DisplayAccount {
    fn from(account: &Account) -> Self {
        Self {
            id: account.id.clone(),
            name: account.name.clone(),
            account_type: account.account_type.clone(),
            on_budget: account.on_budget,
            balance: format_milliunits(account.balance),
            cleared_balance: format_milliunits(account.cleared_balance),
            uncleared_balance: format_milliunits(account.uncleared_balance),
        }
    }
}

#[derive(Deserialize)]
pub struct AccountResponse {
    pub data: AccountData,
}

#[derive(Deserialize)]
pub struct AccountData {
    pub account: Account,
}

// ============================================================================
// Categories
// ============================================================================

#[derive(Deserialize)]
pub struct CategoryGroupsResponse {
    pub data: CategoryGroupsData,
}

#[derive(Deserialize)]
pub struct CategoryGroupsData {
    pub category_groups: Vec<CategoryGroup>,
}

#[derive(Deserialize, Serialize)]
pub struct CategoryGroup {
    pub id: String,
    pub name: String,
    pub hidden: bool,
    pub deleted: bool,
    pub categories: Vec<Category>,
}

#[derive(Deserialize, Serialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub hidden: bool,
    pub deleted: bool,
    #[serde(rename = "type")]
    pub category_type: Option<String>,
}

#[derive(Deserialize)]
pub struct CategoryResponse {
    pub data: CategoryData,
}

#[derive(Deserialize)]
pub struct CategoryData {
    pub category: Category,
}

#[derive(Deserialize)]
pub struct MonthCategoryResponse {
    pub data: MonthCategoryData,
}

#[derive(Deserialize)]
pub struct MonthCategoryData {
    pub month_category: MonthCategory,
}

#[derive(Deserialize)]
pub struct MonthCategory {
    pub id: String,
    pub category_id: String,
    pub month: String,
    pub activity: i64,
    pub budgeted: i64,
}

// ============================================================================
// Payees
// ============================================================================

#[derive(Deserialize)]
pub struct PayeesResponse {
    pub data: PayeesData,
}

#[derive(Deserialize)]
pub struct PayeesData {
    pub payees: Vec<Payee>,
}

#[derive(Deserialize, Serialize)]
pub struct Payee {
    pub id: String,
    pub name: String,
    pub deleted: bool,
}

#[derive(Deserialize)]
pub struct PayeeResponse {
    pub data: PayeeData,
}

#[derive(Deserialize)]
pub struct PayeeData {
    pub payee: Payee,
}

// ============================================================================
// Months
// ============================================================================

#[derive(Deserialize)]
pub struct MonthsResponse {
    pub data: MonthsData,
}

#[derive(Deserialize)]
pub struct MonthsData {
    pub months: Vec<Month>,
}

#[derive(Deserialize, Serialize)]
pub struct Month {
    pub month: String,
    pub note: Option<String>,
    pub income: i64,
    pub budgeted: i64,
    pub activity: i64,
}

#[derive(Deserialize)]
pub struct MonthResponse {
    pub data: MonthData,
}

#[derive(Deserialize)]
pub struct MonthData {
    pub month: Month,
}

// ============================================================================
// Transactions
// ============================================================================

#[derive(Deserialize)]
pub struct TransactionsResponse {
    pub data: TransactionsData,
}

#[derive(Deserialize)]
pub struct TransactionsData {
    pub transactions: Vec<Transaction>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Transaction {
    pub id: String,
    pub date: String,
    pub amount: i64,
    pub amount_formatted: Option<String>,
    pub amount_currency: Option<f64>,
    pub memo: Option<String>,
    pub cleared: ClearedStatus,
    pub approved: bool,
    pub flag_color: Option<String>,
    pub flag_name: Option<String>,
    pub account_id: String,
    pub account_name: Option<String>,
    pub payee_id: Option<String>,
    pub payee_name: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub transfer_account_id: Option<String>,
    pub transfer_transaction_id: Option<String>,
    pub matched_transaction_id: Option<String>,
    pub import_id: Option<String>,
    pub import_payee_name: Option<String>,
    pub import_payee_name_original: Option<String>,
    pub debt_transaction_type: Option<String>,
    pub deleted: bool,
    pub subtransactions: Vec<SubTransaction>,
}

/// Display-friendly transaction with formatted amount.
#[derive(Serialize)]
pub struct DisplayTransaction {
    pub id: String,
    pub date: String,
    pub amount: String,
    pub memo: Option<String>,
    pub cleared: String,
    pub account_name: Option<String>,
    pub payee_name: Option<String>,
    pub category_name: Option<String>,
}

impl From<&Transaction> for DisplayTransaction {
    fn from(tx: &Transaction) -> Self {
        Self {
            id: tx.id.clone(),
            date: tx.date.clone(),
            amount: tx.amount_formatted.clone().unwrap_or_else(|| format_milliunits(tx.amount)),
            memo: tx.memo.clone(),
            cleared: format!("{:?}", tx.cleared),
            account_name: tx.account_name.clone(),
            payee_name: tx.payee_name.clone(),
            category_name: tx.category_name.clone(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SubTransaction {
    pub id: String,
    pub transaction_id: String,
    pub amount: i64,
    pub memo: Option<String>,
    pub payee_id: Option<String>,
    pub payee_name: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub transfer_account_id: Option<String>,
    pub transfer_transaction_id: Option<String>,
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ClearedStatus {
    Cleared,
    Uncleared,
    Reconciled,
}

impl<'de> Deserialize<'de> for ClearedStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ClearedVisitor;
        impl<'de> Visitor<'de> for ClearedVisitor {
            type Value = ClearedStatus;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string: cleared, uncleared, or reconciled")
            }
            fn visit_str<E>(self, value: &str) -> Result<ClearedStatus, E>
            where
                E: de::Error,
            {
                match value {
                    "cleared" => Ok(ClearedStatus::Cleared),
                    "uncleared" => Ok(ClearedStatus::Uncleared),
                    "reconciled" => Ok(ClearedStatus::Reconciled),
                    _ => Err(de::Error::unknown_variant(
                        value,
                        &["cleared", "uncleared", "reconciled"],
                    )),
                }
            }
        }
        deserializer.deserialize_str(ClearedVisitor)
    }
}

#[derive(Deserialize)]
pub struct TransactionResponse {
    pub data: TransactionData,
}

#[derive(Deserialize)]
pub struct TransactionData {
    pub transaction: Transaction,
}

// ============================================================================
// Scheduled Transactions
// ============================================================================

#[derive(Deserialize)]
pub struct ScheduledTransactionsResponse {
    pub data: ScheduledTransactionsData,
}

#[derive(Deserialize)]
pub struct ScheduledTransactionsData {
    pub scheduled_transactions: Vec<ScheduledTransaction>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ScheduledTransaction {
    pub id: String,
    pub date_first: Option<String>,
    pub date_next: Option<String>,
    pub frequency: String,
    pub amount: i64,
    pub amount_formatted: Option<String>,
    pub amount_currency: Option<f64>,
    pub memo: Option<String>,
    pub flag_color: Option<String>,
    pub flag_name: Option<String>,
    pub account_id: String,
    pub account_name: Option<String>,
    pub payee_id: Option<String>,
    pub payee_name: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub transfer_account_id: Option<String>,
    pub deleted: bool,
    pub subtransactions: Vec<SubTransaction>,
}

#[derive(Deserialize)]
pub struct ScheduledTransactionResponse {
    pub data: ScheduledTransactionData,
}

#[derive(Deserialize)]
pub struct ScheduledTransactionData {
    pub scheduled_transaction: ScheduledTransaction,
}

// ============================================================================
// Money Movements
// ============================================================================

#[derive(Deserialize)]
pub struct MoneyMovementsResponse {
    pub data: MoneyMovementsData,
}

#[derive(Deserialize)]
pub struct MoneyMovementsData {
    pub money_movements: Vec<MoneyMovement>,
}

#[derive(Deserialize)]
pub struct MoneyMovement {
    pub id: String,
    pub month: String,
    pub moved_at: Option<String>,
    pub note: Option<String>,
    pub money_movement_group_id: Option<String>,
    pub performed_by_user_id: Option<String>,
    pub from_category_id: Option<String>,
    pub to_category_id: Option<String>,
    pub amount: i64,
    pub amount_formatted: Option<String>,
    pub amount_currency: Option<f64>,
    pub deleted: bool,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert milliunits to dollars (f64).
pub fn milliunits_to_dollars(milliunits: i64) -> f64 {
    milliunits as f64 / 1000.0
}

/// Format milliunits as a display string.
pub fn format_milliunits(milliunits: i64) -> String {
    let dollars = milliunits_to_dollars(milliunits);
    if dollars < 0.0 {
        format!("-${:.2}", dollars.abs())
    } else {
        format!("${:.2}", dollars)
    }
}

/// Aggregate transactions by payee name, returning (name, total_amount).
pub fn aggregate_by_payee(transactions: &[Transaction], category_id: &str) -> HashMap<String, i64> {
    let mut totals: HashMap<String, i64> = HashMap::new();
    for tx in transactions {
        if tx.deleted {
            continue;
        }
        if let Some(ref cid) = tx.category_id {
            if cid == category_id {
                let name = tx.payee_name.as_deref().unwrap_or("Unknown").to_string();
                *totals.entry(name).or_insert(0) += tx.amount;
            }
        }
    }
    totals
}

/// Calculate total spending for a category.
pub fn calculate_category_spending(transactions: &[Transaction], category_id: &str) -> i64 {
    transactions
        .iter()
        .filter(|tx| !tx.deleted)
        .filter(|tx| tx.category_id.as_deref() == Some(category_id))
        .map(|tx| tx.amount)
        .sum()
}

/// Filter transactions by date range, returning owned transactions.
pub fn filter_transactions_by_date(
    transactions: &[Transaction],
    start: &str,
    end: &str,
) -> Vec<Transaction> {
    transactions
        .iter()
        .filter(|tx| !tx.deleted && tx.date.as_str() >= start && tx.date.as_str() <= end)
        .cloned()
        .collect()
}

/// Find a category by name (case-insensitive).
pub fn find_category(groups: &[CategoryGroup], name: &str) -> Option<(String, String)> {
    let name_lower = name.to_lowercase();
    for group in groups {
        for cat in &group.categories {
            if !cat.hidden && !cat.deleted && cat.name.to_lowercase().contains(&name_lower) {
                return Some((cat.id.clone(), cat.name.clone()));
            }
        }
    }
    None
}

/// Find a payee by name (case-insensitive).
pub fn find_payee(payees: &[Payee], name: &str) -> Option<(String, String)> {
    let name_lower = name.to_lowercase();
    for payee in payees {
        if !payee.deleted && payee.name.to_lowercase().contains(&name_lower) {
            return Some((payee.id.clone(), payee.name.clone()));
        }
    }
    None
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // milliunits_to_dollars tests
    // ========================================================================

    #[test]
    fn milliunits_to_dollars_positive_returns_positive_value() {
        // Arrange
        let milliunits = 12345i64;
        // Act
        let result = milliunits_to_dollars(milliunits);
        // Assert
        assert_eq!(result, 12.345);
    }

    #[test]
    fn milliunits_to_dollars_negative_returns_negative_value() {
        // Arrange
        let milliunits = -12345i64;
        // Act
        let result = milliunits_to_dollars(milliunits);
        // Assert
        assert_eq!(result, -12.345);
    }

    #[test]
    fn milliunits_to_dollars_zero_returns_zero() {
        // Arrange
        let milliunits = 0i64;
        // Act
        let result = milliunits_to_dollars(milliunits);
        // Assert
        assert_eq!(result, 0.0);
    }

    #[test]
    fn milliunits_to_dollars_large_value_handles_correctly() {
        // Arrange
        let milliunits = 1_000_000_000i64;
        // Act
        let result = milliunits_to_dollars(milliunits);
        // Assert
        assert_eq!(result, 1_000_000.0);
    }

    #[test]
    fn milliunits_to_dollars_fractional_cents_rounds() {
        // Arrange
        let milliunits = 1234567i64;
        // Act
        let result = milliunits_to_dollars(milliunits);
        // Assert
        assert_eq!(result, 1234.567);
    }

    // ========================================================================
    // format_milliunits tests
    // ========================================================================

    #[test]
    fn format_milliunits_positive_returns_dollar_format() {
        // Arrange
        let milliunits = 12345i64;
        // Act
        let result = format_milliunits(milliunits);
        // Assert
        assert_eq!(result, "$12.35");
    }

    #[test]
    fn format_milliunits_negative_returns_negative_dollar_format() {
        // Arrange
        let milliunits = -12345i64;
        // Act
        let result = format_milliunits(milliunits);
        // Assert
        assert_eq!(result, "-$12.35");
    }

    #[test]
    fn format_milliunits_zero_returns_zero_dollar_format() {
        // Arrange
        let milliunits = 0i64;
        // Act
        let result = format_milliunits(milliunits);
        // Assert
        assert_eq!(result, "$0.00");
    }

    #[test]
    fn format_milliunits_single_dollar_returns_correct_format() {
        // Arrange
        let milliunits = 1000i64;
        // Act
        let result = format_milliunits(milliunits);
        // Assert
        assert_eq!(result, "$1.00");
    }

    #[test]
    fn format_milliunits_large_value_formats_with_commas() {
        // Arrange
        let milliunits = 1_234_567i64;
        // Act
        let result = format_milliunits(milliunits);
        // Assert
        assert_eq!(result, "$1234.57");
    }

    // ========================================================================
    // Helper functions for creating test data
    // ========================================================================

    fn make_transaction(
        id: &str,
        amount: i64,
        date: &str,
        category_id: Option<String>,
        payee_name: Option<String>,
    ) -> Transaction {
        Transaction {
            id: id.to_string(),
            date: date.to_string(),
            amount,
            amount_formatted: None,
            amount_currency: None,
            memo: None,
            cleared: ClearedStatus::Cleared,
            approved: true,
            flag_color: None,
            flag_name: None,
            account_id: "account-1".to_string(),
            account_name: None,
            payee_id: None,
            payee_name,
            category_id,
            category_name: None,
            transfer_account_id: None,
            transfer_transaction_id: None,
            matched_transaction_id: None,
            import_id: None,
            import_payee_name: None,
            import_payee_name_original: None,
            debt_transaction_type: None,
            deleted: false,
            subtransactions: vec![],
        }
    }

    fn make_category(id: &str, name: &str) -> Category {
        Category {
            id: id.to_string(),
            name: name.to_string(),
            hidden: false,
            deleted: false,
            category_type: None,
        }
    }

    fn make_category_group(id: &str, name: &str, categories: Vec<Category>) -> CategoryGroup {
        CategoryGroup {
            id: id.to_string(),
            name: name.to_string(),
            hidden: false,
            deleted: false,
            categories,
        }
    }

    fn make_payee(id: &str, name: &str, deleted: bool) -> Payee {
        Payee {
            id: id.to_string(),
            name: name.to_string(),
            deleted,
        }
    }

    // ========================================================================
    // aggregate_by_payee tests
    // ========================================================================

    #[test]
    fn aggregate_by_payee_groups_transactions_by_payee() {
        // Arrange
        let transactions = vec![
            make_transaction(
                "tx1",
                -5000,
                "2026-04-01",
                Some("cat-1".to_string()),
                Some("Grocery Store".to_string()),
            ),
            make_transaction(
                "tx2",
                -3000,
                "2026-04-02",
                Some("cat-1".to_string()),
                Some("Grocery Store".to_string()),
            ),
            make_transaction(
                "tx3",
                -2000,
                "2026-04-03",
                Some("cat-1".to_string()),
                Some("Gas Station".to_string()),
            ),
        ];
        // Act
        let result = aggregate_by_payee(&transactions, "cat-1");
        // Assert
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("Grocery Store"), Some(&-8000));
        assert_eq!(result.get("Gas Station"), Some(&-2000));
    }

    #[test]
    fn aggregate_by_payee_filters_deleted_transactions() {
        // Arrange
        let mut tx1 = make_transaction(
            "tx1",
            -5000,
            "2026-04-01",
            Some("cat-1".to_string()),
            Some("Grocery Store".to_string()),
        );
        tx1.deleted = true;
        let tx2 = make_transaction(
            "tx2",
            -3000,
            "2026-04-02",
            Some("cat-1".to_string()),
            Some("Grocery Store".to_string()),
        );
        let transactions = vec![tx1, tx2];
        // Act
        let result = aggregate_by_payee(&transactions, "cat-1");
        // Assert
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("Grocery Store"), Some(&-3000));
    }

    #[test]
    fn aggregate_by_payee_filters_by_category_id() {
        // Arrange
        let transactions = vec![
            make_transaction(
                "tx1",
                -5000,
                "2026-04-01",
                Some("cat-1".to_string()),
                Some("Store A".to_string()),
            ),
            make_transaction(
                "tx2",
                -3000,
                "2026-04-02",
                Some("cat-2".to_string()),
                Some("Store B".to_string()),
            ),
        ];
        // Act
        let result = aggregate_by_payee(&transactions, "cat-1");
        // Assert
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("Store A"), Some(&-5000));
    }

    #[test]
    fn aggregate_by_payee_handles_missing_payee_name() {
        // Arrange
        let tx = make_transaction("tx1", -5000, "2026-04-01", Some("cat-1".to_string()), None);
        let transactions = vec![tx];
        // Act
        let result = aggregate_by_payee(&transactions, "cat-1");
        // Assert
        assert_eq!(result.get("Unknown"), Some(&-5000));
    }

    #[test]
    fn aggregate_by_payee_empty_transactions_returns_empty_map() {
        // Arrange
        let transactions: Vec<Transaction> = vec![];
        // Act
        let result = aggregate_by_payee(&transactions, "cat-1");
        // Assert
        assert!(result.is_empty());
    }

    // ========================================================================
    // calculate_category_spending tests
    // ========================================================================

    #[test]
    fn calculate_category_spending_sums_all_matching_transactions() {
        // Arrange
        let transactions = vec![
            make_transaction(
                "tx1",
                -5000,
                "2026-04-01",
                Some("cat-1".to_string()),
                Some("Store".to_string()),
            ),
            make_transaction(
                "tx2",
                -3000,
                "2026-04-02",
                Some("cat-1".to_string()),
                Some("Store".to_string()),
            ),
            make_transaction(
                "tx3",
                -2000,
                "2026-04-03",
                Some("cat-2".to_string()),
                Some("Store".to_string()),
            ),
        ];
        // Act
        let result = calculate_category_spending(&transactions, "cat-1");
        // Assert
        assert_eq!(result, -8000);
    }

    #[test]
    fn calculate_category_spending_excludes_deleted_transactions() {
        // Arrange
        let mut tx1 = make_transaction(
            "tx1",
            -5000,
            "2026-04-01",
            Some("cat-1".to_string()),
            Some("Store".to_string()),
        );
        tx1.deleted = true;
        let tx2 = make_transaction(
            "tx2",
            -3000,
            "2026-04-02",
            Some("cat-1".to_string()),
            Some("Store".to_string()),
        );
        let transactions = vec![tx1, tx2];
        // Act
        let result = calculate_category_spending(&transactions, "cat-1");
        // Assert
        assert_eq!(result, -3000);
    }

    #[test]
    fn calculate_category_spending_zero_for_no_matches() {
        // Arrange
        let transactions = vec![make_transaction(
            "tx1",
            -5000,
            "2026-04-01",
            Some("cat-1".to_string()),
            Some("Store".to_string()),
        )];
        // Act
        let result = calculate_category_spending(&transactions, "cat-nonexistent");
        // Assert
        assert_eq!(result, 0);
    }

    #[test]
    fn calculate_category_spending_empty_transactions_returns_zero() {
        // Arrange
        let transactions: Vec<Transaction> = vec![];
        // Act
        let result = calculate_category_spending(&transactions, "cat-1");
        // Assert
        assert_eq!(result, 0);
    }

    // ========================================================================
    // filter_transactions_by_date tests
    // ========================================================================

    #[test]
    fn filter_transactions_by_date_returns_matching_transactions() {
        // Arrange
        let transactions = vec![
            make_transaction("tx1", -5000, "2026-04-01", Some("cat-1".to_string()), None),
            make_transaction("tx2", -3000, "2026-04-15", Some("cat-1".to_string()), None),
            make_transaction("tx3", -2000, "2026-05-01", Some("cat-1".to_string()), None),
        ];
        // Act
        let result = filter_transactions_by_date(&transactions, "2026-04-01", "2026-04-30");
        // Assert
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_transactions_by_date_excludes_deleted() {
        // Arrange
        let mut tx1 = make_transaction("tx1", -5000, "2026-04-01", Some("cat-1".to_string()), None);
        tx1.deleted = true;
        let tx2 = make_transaction("tx2", -3000, "2026-04-15", Some("cat-1".to_string()), None);
        let transactions = vec![tx1, tx2];
        // Act
        let result = filter_transactions_by_date(&transactions, "2026-04-01", "2026-04-30");
        // Assert
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_transactions_by_date_includes_boundary_dates() {
        // Arrange
        let tx1 = make_transaction("tx1", -5000, "2026-04-01", Some("cat-1".to_string()), None);
        let tx2 = make_transaction("tx2", -3000, "2026-04-30", Some("cat-1".to_string()), None);
        let transactions = vec![tx1, tx2];
        // Act
        let result = filter_transactions_by_date(&transactions, "2026-04-01", "2026-04-30");
        // Assert
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_transactions_by_date_returns_empty_for_no_matches() {
        // Arrange
        let transactions = vec![make_transaction(
            "tx1",
            -5000,
            "2026-06-01",
            Some("cat-1".to_string()),
            None,
        )];
        // Act
        let result = filter_transactions_by_date(&transactions, "2026-04-01", "2026-04-30");
        // Assert
        assert!(result.is_empty());
    }

    // ========================================================================
    // find_category tests
    // ========================================================================

    #[test]
    fn find_category_finds_exact_match() {
        // Arrange
        let categories = vec![make_category("cat-1", "Groceries")];
        let group = make_category_group("group-1", "Monthly", categories);
        let groups = vec![group];
        // Act
        let result = find_category(&groups, "Groceries");
        // Assert
        assert!(result.is_some());
        let (id, name) = result.unwrap();
        assert_eq!(id, "cat-1");
        assert_eq!(name, "Groceries");
    }

    #[test]
    fn find_category_finds_case_insensitive() {
        // Arrange
        let categories = vec![make_category("cat-1", "Groceries")];
        let group = make_category_group("group-1", "Monthly", categories);
        let groups = vec![group];
        // Act
        let result = find_category(&groups, "groceries");
        // Assert
        assert!(result.is_some());
    }

    #[test]
    fn find_category_returns_partial_match() {
        // Arrange
        let categories = vec![make_category("cat-1", "Grocery Stores")];
        let group = make_category_group("group-1", "Shopping", categories);
        let groups = vec![group];
        // Act
        let result = find_category(&groups, "grocery");
        // Assert
        assert!(result.is_some());
    }

    #[test]
    fn find_category_excludes_hidden_categories() {
        // Arrange
        let mut categories = vec![make_category("cat-1", "Hidden Category")];
        categories[0].hidden = true;
        let group = make_category_group("group-1", "Monthly", categories);
        let groups = vec![group];
        // Act
        let result = find_category(&groups, "hidden");
        // Assert
        assert!(result.is_none());
    }

    #[test]
    fn find_category_excludes_deleted_categories() {
        // Arrange
        let mut categories = vec![make_category("cat-1", "Deleted Category")];
        categories[0].deleted = true;
        let group = make_category_group("group-1", "Monthly", categories);
        let groups = vec![group];
        // Act
        let result = find_category(&groups, "deleted");
        // Assert
        assert!(result.is_none());
    }

    #[test]
    fn find_category_returns_none_for_no_match() {
        // Arrange
        let categories = vec![make_category("cat-1", "Groceries")];
        let group = make_category_group("group-1", "Monthly", categories);
        let groups = vec![group];
        // Act
        let result = find_category(&groups, "NonExistent");
        // Assert
        assert!(result.is_none());
    }

    // ========================================================================
    // find_payee tests
    // ========================================================================

    #[test]
    fn find_payee_finds_exact_match() {
        // Arrange
        let payees = vec![make_payee("payee-1", "Amazon", false)];
        // Act
        let result = find_payee(&payees, "Amazon");
        // Assert
        assert!(result.is_some());
        let (id, name) = result.unwrap();
        assert_eq!(id, "payee-1");
        assert_eq!(name, "Amazon");
    }

    #[test]
    fn find_payee_finds_case_insensitive() {
        // Arrange
        let payees = vec![make_payee("payee-1", "Amazon", false)];
        // Act
        let result = find_payee(&payees, "AMAZON");
        // Assert
        assert!(result.is_some());
    }

    #[test]
    fn find_payee_returns_partial_match() {
        // Arrange
        let payees = vec![make_payee("payee-1", "Whole Foods Market", false)];
        // Act
        let result = find_payee(&payees, "whole");
        // Assert
        assert!(result.is_some());
    }

    #[test]
    fn find_payee_excludes_deleted_payees() {
        // Arrange
        let payees = vec![make_payee("payee-1", "Deleted Store", true)];
        // Act
        let result = find_payee(&payees, "deleted");
        // Assert
        assert!(result.is_none());
    }

    #[test]
    fn find_payee_returns_none_for_no_match() {
        // Arrange
        let payees = vec![make_payee("payee-1", "Amazon", false)];
        // Act
        let result = find_payee(&payees, "NonExistent");
        // Assert
        assert!(result.is_none());
    }
}
