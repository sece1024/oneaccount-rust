use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CategoryType {
    Income,
    Expense,
}

impl std::fmt::Display for CategoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CategoryType::Income => write!(f, "income"),
            CategoryType::Expense => write!(f, "expense"),
        }
    }
}

impl std::str::FromStr for CategoryType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "income" => Ok(CategoryType::Income),
            "expense" => Ok(CategoryType::Expense),
            _ => Err(format!("未知分类类型: {s}")),
        }
    }
}

impl CategoryType {
    pub fn display_name(&self) -> &'static str {
        match self {
            CategoryType::Income => "收入",
            CategoryType::Expense => "支出",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub category_type: CategoryType,
    pub icon: Option<String>,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewCategory {
    pub name: String,
    pub category_type: CategoryType,
    pub icon: Option<String>,
    pub parent_id: Option<i64>,
}
