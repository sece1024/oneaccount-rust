use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Income,
    Expense,
    Transfer,
}

impl std::fmt::Display for TransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionType::Income => write!(f, "income"),
            TransactionType::Expense => write!(f, "expense"),
            TransactionType::Transfer => write!(f, "transfer"),
        }
    }
}

impl std::str::FromStr for TransactionType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "income" => Ok(TransactionType::Income),
            "expense" => Ok(TransactionType::Expense),
            "transfer" => Ok(TransactionType::Transfer),
            _ => Err(format!("未知账目类型: {s}")),
        }
    }
}

impl TransactionType {
    pub fn display_name(&self) -> &'static str {
        match self {
            TransactionType::Income => "收入",
            TransactionType::Expense => "支出",
            TransactionType::Transfer => "转账",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: i64,
    pub amount: f64,
    pub transaction_type: TransactionType,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub account_id: i64,
    pub account_name: Option<String>,
    pub to_account_id: Option<i64>,
    pub to_account_name: Option<String>,
    pub date: String,
    pub note: Option<String>,
    pub is_large: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewTransaction {
    pub amount: f64,
    pub transaction_type: TransactionType,
    pub category_id: Option<i64>,
    pub account_id: i64,
    pub to_account_id: Option<i64>,
    pub date: String,
    pub note: Option<String>,
    #[serde(default)]
    pub is_large: bool,
}

/// 查询过滤条件
#[derive(Debug, Default, Clone)]
pub struct TransactionFilter {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub category_id: Option<i64>,
    pub account_id: Option<i64>,
    pub transaction_type: Option<TransactionType>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub is_large_only: bool,
}
