use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Cash,
    Bank,
    Wechat,
    Alipay,
    Stock,
    Crypto,
    SocialInsurance,
    Fund,
    /// 信用账户：信用卡、花呗、美团月付等负债账户
    CreditCard,
}

impl std::fmt::Display for AccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountType::Cash => write!(f, "cash"),
            AccountType::Bank => write!(f, "bank"),
            AccountType::Wechat => write!(f, "wechat"),
            AccountType::Alipay => write!(f, "alipay"),
            AccountType::Stock => write!(f, "stock"),
            AccountType::Crypto => write!(f, "crypto"),
            AccountType::SocialInsurance => write!(f, "social_insurance"),
            AccountType::Fund => write!(f, "fund"),
            AccountType::CreditCard => write!(f, "credit_card"),
        }
    }
}

impl std::str::FromStr for AccountType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cash" | "现金" => Ok(AccountType::Cash),
            "bank" | "银行" | "银行卡" => Ok(AccountType::Bank),
            "wechat" | "微信" => Ok(AccountType::Wechat),
            "alipay" | "支付宝" => Ok(AccountType::Alipay),
            "stock" | "股票" => Ok(AccountType::Stock),
            "crypto" | "虚拟货币" | "加密货币" => Ok(AccountType::Crypto),
            "social_insurance" | "社保" => Ok(AccountType::SocialInsurance),
            "fund" | "基金" => Ok(AccountType::Fund),
            "credit_card" | "信用卡" | "花呗" | "月付" => Ok(AccountType::CreditCard),
            _ => Err(format!("未知账户类型: {s}")),
        }
    }
}

impl AccountType {
    pub fn display_name(&self) -> &'static str {
        match self {
            AccountType::Cash => "现金",
            AccountType::Bank => "银行卡",
            AccountType::Wechat => "微信",
            AccountType::Alipay => "支付宝",
            AccountType::Stock => "股票",
            AccountType::Crypto => "虚拟货币",
            AccountType::SocialInsurance => "社保",
            AccountType::Fund => "基金",
            AccountType::CreditCard => "信用/负债",
        }
    }

    pub fn all() -> Vec<AccountType> {
        vec![
            AccountType::Cash,
            AccountType::Bank,
            AccountType::Wechat,
            AccountType::Alipay,
            AccountType::Stock,
            AccountType::Crypto,
            AccountType::SocialInsurance,
            AccountType::Fund,
            AccountType::CreditCard,
        ]
    }

    /// 默认是否为活动资金（可随时提取）
    pub fn default_liquid(&self) -> bool {
        !matches!(self, AccountType::Stock | AccountType::Crypto | AccountType::SocialInsurance | AccountType::Fund)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub account_type: AccountType,
    pub currency: String,
    pub balance: f64,
    pub is_liquid: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewAccount {
    pub name: String,
    pub account_type: AccountType,
    pub currency: String,
    pub initial_balance: f64,
    #[serde(default = "default_true")]
    pub is_liquid: bool,
}

fn default_true() -> bool { true }

impl Default for NewAccount {
    fn default() -> Self {
        Self {
            name: String::new(),
            account_type: AccountType::Cash,
            currency: "CNY".to_string(),
            initial_balance: 0.0,
            is_liquid: true,
        }
    }
}
