use rusqlite::{params, Connection, OptionalExtension};
use std::str::FromStr;

use crate::error::{AppError, Result};
use crate::models::transaction::{NewTransaction, Transaction, TransactionFilter, TransactionType};

const SELECT_COLS: &str = "
    SELECT t.id, t.amount, t.transaction_type,
           t.category_id, c.name,
           t.account_id, a.name,
           t.to_account_id, ta.name,
           t.date, t.note, t.created_at, t.updated_at, t.is_large
    FROM transactions t
    LEFT JOIN categories c  ON t.category_id   = c.id
    LEFT JOIN accounts   a  ON t.account_id    = a.id
    LEFT JOIN accounts   ta ON t.to_account_id = ta.id
";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Transaction> {
    let type_str: String = row.get(2)?;
    let is_large: i64 = row.get(13)?;
    Ok(Transaction {
        id: row.get(0)?,
        amount: row.get(1)?,
        transaction_type: TransactionType::from_str(&type_str).unwrap_or(TransactionType::Expense),
        category_id: row.get(3)?,
        category_name: row.get(4)?,
        account_id: row.get(5)?,
        account_name: row.get(6)?,
        to_account_id: row.get(7)?,
        to_account_name: row.get(8)?,
        date: row.get(9)?,
        note: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
        is_large: is_large != 0,
    })
}

pub struct TransactionDao;

impl TransactionDao {
    pub fn create(conn: &Connection, req: &NewTransaction) -> Result<Transaction> {
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO transactions
                 (amount, transaction_type, category_id, account_id, to_account_id,
                  date, note, is_large, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                req.amount,
                req.transaction_type.to_string(),
                req.category_id,
                req.account_id,
                req.to_account_id,
                req.date,
                req.note,
                req.is_large as i64,
                now,
            ],
        )?;
        let id = conn.last_insert_rowid();
        Self::find_by_id(conn, id)?.ok_or_else(|| AppError::NotFound("Transaction".into()))
    }

    pub fn find_by_id(conn: &Connection, id: i64) -> Result<Option<Transaction>> {
        let sql = format!("{SELECT_COLS} WHERE t.id = ?1");
        let mut stmt = conn.prepare(&sql)?;
        Ok(stmt.query_row(params![id], map_row).optional()?)
    }

    /// 动态过滤查询
    pub fn find_with_filter(conn: &Connection, filter: &TransactionFilter) -> Result<Vec<Transaction>> {
        let large_filter = if filter.is_large_only { Some(1i64) } else { None };
        let sql = format!(
            "{SELECT_COLS}
             WHERE (?1 IS NULL OR t.date >= ?1)
               AND (?2 IS NULL OR t.date <= ?2)
               AND (?3 IS NULL OR t.category_id = ?3)
               AND (?4 IS NULL OR t.account_id = ?4)
               AND (?5 IS NULL OR t.transaction_type = ?5)
               AND (?6 IS NULL OR t.is_large = ?6)
             ORDER BY t.date DESC, t.id DESC
             LIMIT COALESCE(?7, 9999999)
             OFFSET COALESCE(?8, 0)"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![
                filter.start_date,
                filter.end_date,
                filter.category_id,
                filter.account_id,
                filter.transaction_type.as_ref().map(|t| t.to_string()),
                large_filter,
                filter.limit,
                filter.offset,
            ],
            map_row,
        )?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    #[allow(dead_code)]
    pub fn recent(conn: &Connection, limit: i64) -> Result<Vec<Transaction>> {
        Self::find_with_filter(
            conn,
            &TransactionFilter {
                limit: Some(limit),
                ..Default::default()
            },
        )
    }

    pub fn delete(conn: &Connection, id: i64) -> Result<()> {
        conn.execute("DELETE FROM transactions WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn update(conn: &Connection, id: i64, req: &NewTransaction) -> Result<Transaction> {
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "UPDATE transactions SET
                 amount = ?1, transaction_type = ?2, category_id = ?3,
                 account_id = ?4, to_account_id = ?5, date = ?6,
                 note = ?7, is_large = ?8, updated_at = ?9
             WHERE id = ?10",
            params![
                req.amount,
                req.transaction_type.to_string(),
                req.category_id,
                req.account_id,
                req.to_account_id,
                req.date,
                req.note,
                req.is_large as i64,
                now,
                id,
            ],
        )?;
        Self::find_by_id(conn, id)?.ok_or_else(|| AppError::NotFound("Transaction".into()))
    }

    /// 月度收支汇总，返回 (income, expense)
    pub fn monthly_summary(conn: &Connection, year: i32, month: u32) -> Result<(f64, f64)> {
        let start = format!("{year:04}-{month:02}-01");
        let end = if month == 12 {
            format!("{:04}-01-01", year + 1)
        } else {
            format!("{year:04}-{:02}-01", month + 1)
        };

        let income: f64 = conn.query_row(
            "SELECT COALESCE(SUM(amount), 0.0) FROM transactions
             WHERE transaction_type = 'income' AND date >= ?1 AND date < ?2",
            params![start, end],
            |r| r.get(0),
        )?;
        let expense: f64 = conn.query_row(
            "SELECT COALESCE(SUM(amount), 0.0) FROM transactions
             WHERE transaction_type = 'expense' AND date >= ?1 AND date < ?2",
            params![start, end],
            |r| r.get(0),
        )?;
        Ok((income, expense))
    }

    /// 分类统计（支出），返回 Vec<(分类名, 金额)>
    pub fn category_stats(
        conn: &Connection,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<(String, f64)>> {
        let mut stmt = conn.prepare(
            "SELECT COALESCE(c.name, '未分类'), SUM(t.amount)
             FROM transactions t
             LEFT JOIN categories c ON t.category_id = c.id
             WHERE t.transaction_type = 'expense'
               AND t.date >= ?1 AND t.date <= ?2
             GROUP BY t.category_id
             ORDER BY SUM(t.amount) DESC",
        )?;
        let rows = stmt.query_map(params![start_date, end_date], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dao::AccountDao;
    use crate::models::account::{AccountType, NewAccount};

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        crate::db::migrations::seed_default_data(&conn).unwrap();
        conn
    }

    fn mk_account(conn: &Connection, name: &str) -> i64 {
        AccountDao::create(
            conn,
            &NewAccount {
                name: name.into(),
                account_type: AccountType::Cash,
                currency: "CNY".into(),
                initial_balance: 1000.0,
            is_liquid: true,
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn test_create_and_find() {
        let conn = setup();
        let acc_id = mk_account(&conn, "测试账户");

        let req = NewTransaction {
            amount: 35.5,
            transaction_type: TransactionType::Expense,
            category_id: None,
            account_id: acc_id,
            to_account_id: None,
            date: "2024-01-15".into(),
            note: Some("午饭".into()),
            is_large: false,
        };
        let tx = TransactionDao::create(&conn, &req).unwrap();
        assert_eq!(tx.amount, 35.5);
        assert_eq!(tx.account_id, acc_id);

        let found = TransactionDao::find_by_id(&conn, tx.id).unwrap();
        assert!(found.is_some());
    }

    #[test]
    fn test_filter_by_date() {
        let conn = setup();
        let acc_id = mk_account(&conn, "账户A");

        for (date, amount) in &[("2024-01-10", 100.0), ("2024-01-20", 200.0), ("2024-02-05", 50.0)] {
            TransactionDao::create(
                &conn,
                &NewTransaction {
                    amount: *amount,
                    transaction_type: TransactionType::Expense,
                    category_id: None,
                    account_id: acc_id,
                    to_account_id: None,
                    date: date.to_string(),
                    note: None,
                    is_large: false,
                },
            )
            .unwrap();
        }

        let filter = TransactionFilter {
            start_date: Some("2024-01-01".into()),
            end_date: Some("2024-01-31".into()),
            ..Default::default()
        };
        let results = TransactionDao::find_with_filter(&conn, &filter).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_monthly_summary() {
        let conn = setup();
        let acc_id = mk_account(&conn, "账户B");

        TransactionDao::create(
            &conn,
            &NewTransaction {
                amount: 5000.0,
                transaction_type: TransactionType::Income,
                category_id: None,
                account_id: acc_id,
                to_account_id: None,
                date: "2024-03-01".into(),
                note: None,
                is_large: false,
            },
        )
        .unwrap();
        TransactionDao::create(
            &conn,
            &NewTransaction {
                amount: 200.0,
                transaction_type: TransactionType::Expense,
                category_id: None,
                account_id: acc_id,
                to_account_id: None,
                date: "2024-03-15".into(),
                note: None,
                is_large: false,
            },
        )
        .unwrap();

        let (income, expense) = TransactionDao::monthly_summary(&conn, 2024, 3).unwrap();
        assert!((income - 5000.0).abs() < 1e-9);
        assert!((expense - 200.0).abs() < 1e-9);
    }
}
