use serde::{Deserialize, Serialize};

fn fmt_balance(b: f64) -> String {
    if b < 0.0 {
        format!("-¥{:.2}", b.abs())
    } else {
        format!("¥{:.2}", b)
    }
}

/// 单账户月结快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSnapshot {
    pub id: i64,
    pub year: i32,
    pub month: u32,
    pub account_id: i64,
    pub account_name: String,
    pub account_type: String,
    pub balance: f64,
    pub note: Option<String>,
    pub created_at: String,
}

/// 某月的全量快照（所有账户汇总）
#[derive(Debug, Clone)]
pub struct MonthlyTotal {
    pub year: i32,
    pub month: u32,
    pub total: f64,
}

/// 为月结表单准备的账户输入项
#[derive(Debug, Clone)]
pub struct MonthlyEntryItem {
    pub account_id: i64,
    pub account_name: String,
    pub account_type: String,
    /// 上次快照余额（可能不存在）
    pub last_balance: Option<f64>,
    /// 用户本次输入
    pub input: String,
    /// 已确认的余额
    pub confirmed_balance: Option<f64>,
}

impl MonthlyEntryItem {
    pub fn display_last(&self) -> String {
        self.last_balance
            .map(|b| fmt_balance(b))
            .unwrap_or_else(|| "—".into())
    }

    pub fn display_new(&self) -> String {
        if let Some(b) = self.confirmed_balance {
            fmt_balance(b)
        } else if !self.input.is_empty() {
            // 用户正在输入，原样显示（前缀 ¥ 或 -¥）
            if self.input.starts_with('-') {
                format!("-¥{}", &self.input[1..])
            } else {
                format!("¥{}", self.input)
            }
        } else {
            String::new()
        }
    }

    pub fn delta_str(&self) -> Option<String> {
        let new = self.confirmed_balance?;
        let old = self.last_balance?;
        let diff = new - old;
        if diff >= 0.0 {
            Some(format!("+¥{diff:.2}"))
        } else {
            Some(format!("-¥{:.2}", diff.abs()))
        }
    }
}
