use std::collections::HashMap;
use std::io::{Read, Write};

use crate::error::{AppError, Result};
use crate::models::transaction::{NewTransaction, TransactionType};
use crate::service::AppService;

/// CSV 字段映射配置（CSV 表头 → 标准字段名）
#[derive(Debug, Default)]
pub struct FieldMapping {
    /// key: 标准字段名, value: CSV 中的表头名
    pub map: HashMap<String, String>,
}

impl FieldMapping {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_default() -> Self {
        let mut m = Self::new();
        // 默认映射（兼容常见银行账单）
        for (std_field, csv_header) in &[
            ("amount", "金额"),
            ("date", "日期"),
            ("note", "备注"),
            ("type", "类型"),
            ("category", "分类"),
            ("account", "账户"),
        ] {
            m.map.insert(std_field.to_string(), csv_header.to_string());
        }
        m
    }

    pub fn get<'a>(&'a self, std_field: &str, record: &'a csv::StringRecord, headers: &csv::StringRecord) -> Option<&'a str> {
        let csv_header = self.map.get(std_field)?;
        let idx = headers.iter().position(|h| h == csv_header.as_str())?;
        record.get(idx)
    }
}

/// 从 Reader 导入 CSV 数据
pub fn import_csv<R: Read>(
    reader: R,
    service: &AppService,
    mapping: &FieldMapping,
    default_account_id: i64,
) -> Result<usize> {
    let mut rdr = csv::Reader::from_reader(reader);
    let headers = rdr.headers()?.clone();
    let mut count = 0;

    for result in rdr.records() {
        let record = result?;

        let amount_str = mapping.get("amount", &record, &headers)
            .ok_or_else(|| AppError::InvalidInput("缺少金额字段".into()))?;
        let amount: f64 = amount_str
            .trim()
            .trim_start_matches('¥')
            .trim_start_matches('￥')
            .replace(',', "")
            .parse()
            .map_err(|_| AppError::InvalidInput(format!("无效金额: {amount_str}")))?;

        let date = mapping
            .get("date", &record, &headers)
            .map(|d| normalize_date(d))
            .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());

        let note = mapping
            .get("note", &record, &headers)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let tx_type = mapping
            .get("type", &record, &headers)
            .and_then(|t| parse_tx_type(t))
            .unwrap_or(TransactionType::Expense);

        service.create_transaction(&NewTransaction {
            amount: amount.abs(),
            transaction_type: tx_type,
            category_id: None,
            account_id: default_account_id,
            to_account_id: None,
            date,
            note,
            is_large: false,
        })?;
        count += 1;
    }

    Ok(count)
}

/// 将账单列表导出为 CSV
pub fn export_csv<W: Write>(writer: W, service: &AppService) -> Result<usize> {
    let txs = service.list_transactions(&crate::models::transaction::TransactionFilter {
        limit: Some(100_000),
        ..Default::default()
    })?;

    let mut wtr = csv::Writer::from_writer(writer);
    wtr.write_record(["日期", "类型", "分类", "金额", "账户", "备注"])?;

    for tx in &txs {
        wtr.write_record([
            tx.date.as_str(),
            tx.transaction_type.display_name(),
            tx.category_name.as_deref().unwrap_or("未分类"),
            &format!("{:.2}", tx.amount),
            tx.account_name.as_deref().unwrap_or(""),
            tx.note.as_deref().unwrap_or(""),
        ])?;
    }
    wtr.flush()?;
    Ok(txs.len())
}

fn normalize_date(s: &str) -> String {
    let s = s.trim();
    // 支持多种日期格式
    for fmt in &["%Y-%m-%d", "%Y/%m/%d", "%Y年%m月%d日", "%m/%d/%Y", "%d/%m/%Y"] {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, fmt) {
            return d.format("%Y-%m-%d").to_string();
        }
    }
    s.to_string()
}

fn parse_tx_type(s: &str) -> Option<TransactionType> {
    match s.trim() {
        "收入" | "income" | "+" => Some(TransactionType::Income),
        "支出" | "expense" | "-" => Some(TransactionType::Expense),
        "转账" | "transfer" => Some(TransactionType::Transfer),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_pool;
    use crate::models::account::{AccountType, NewAccount};

    #[test]
    fn test_import_csv() {
        let pool = std::sync::Arc::new(create_pool(":memory:").unwrap());
        let svc = AppService::new(pool);

        let acc = svc.create_account(&NewAccount {
            name: "测试账户".into(),
            account_type: AccountType::Cash,
            currency: "CNY".into(),
            initial_balance: 0.0,
            is_liquid: true,
        }).unwrap();

        let csv_data = "日期,金额,备注,类型\n2024-01-15,35.5,午饭,支出\n2024-01-16,5000,工资,收入\n";
        let mapping = FieldMapping::with_default();
        let count = import_csv(csv_data.as_bytes(), &svc, &mapping, acc.id).unwrap();
        assert_eq!(count, 2);

        let txs = svc.recent_transactions(10).unwrap();
        assert_eq!(txs.len(), 2);
    }

    #[test]
    fn test_export_csv() {
        let pool = std::sync::Arc::new(create_pool(":memory:").unwrap());
        let svc = AppService::new(pool);

        let acc = svc.create_account(&NewAccount {
            name: "账户".into(),
            account_type: AccountType::Cash,
            currency: "CNY".into(),
            initial_balance: 0.0,
            is_liquid: true,
        }).unwrap();

        svc.create_transaction(&crate::models::transaction::NewTransaction {
            amount: 100.0,
            transaction_type: TransactionType::Expense,
            category_id: None,
            account_id: acc.id,
            to_account_id: None,
            date: "2024-01-01".into(),
            note: Some("测试".into()),
            is_large: false,
        }).unwrap();

        let mut buf = Vec::new();
        let count = export_csv(&mut buf, &svc).unwrap();
        assert_eq!(count, 1);

        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("2024-01-01"));
        assert!(output.contains("100.00"));
    }
}
