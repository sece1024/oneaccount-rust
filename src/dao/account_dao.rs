use rusqlite::{params, Connection, OptionalExtension};
use std::str::FromStr;

use crate::error::{AppError, Result};
use crate::models::account::{Account, AccountType, NewAccount};

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Account> {
    let type_str: String = row.get(2)?;
    Ok(Account {
        id: row.get(0)?,
        name: row.get(1)?,
        account_type: AccountType::from_str(&type_str).unwrap_or(AccountType::Cash),
        currency: row.get(3)?,
        balance: row.get(4)?,
        is_liquid: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub struct AccountDao;

impl AccountDao {
    pub fn create(conn: &Connection, req: &NewAccount) -> Result<Account> {
        let now = chrono::Local::now().to_rfc3339();
        // 若未显式指定，根据账户类型推断默认活动状态
        let is_liquid = if req.is_liquid { 1i64 } else { 0i64 };
        conn.execute(
            "INSERT INTO accounts (name, account_type, currency, balance, is_liquid, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![
                req.name,
                req.account_type.to_string(),
                req.currency,
                req.initial_balance,
                is_liquid,
                now,
            ],
        )?;
        let id = conn.last_insert_rowid();
        Self::find_by_id(conn, id)?.ok_or_else(|| AppError::NotFound("Account".into()))
    }

    pub fn find_by_id(conn: &Connection, id: i64) -> Result<Option<Account>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, account_type, currency, balance, is_liquid, created_at, updated_at
             FROM accounts WHERE id = ?1",
        )?;
        Ok(stmt.query_row(params![id], map_row).optional()?)
    }

    pub fn find_all(conn: &Connection) -> Result<Vec<Account>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, account_type, currency, balance, is_liquid, created_at, updated_at
             FROM accounts ORDER BY is_liquid DESC, name",
        )?;
        let rows = stmt.query_map([], map_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn adjust_balance(conn: &Connection, id: i64, delta: f64) -> Result<()> {
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "UPDATE accounts SET balance = balance + ?1, updated_at = ?2 WHERE id = ?3",
            params![delta, now, id],
        )?;
        Ok(())
    }

    pub fn set_balance(conn: &Connection, id: i64, balance: f64) -> Result<()> {
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "UPDATE accounts SET balance = ?1, updated_at = ?2 WHERE id = ?3",
            params![balance, now, id],
        )?;
        Ok(())
    }

    pub fn delete(conn: &Connection, id: i64) -> Result<()> {
        conn.execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn update_liquid(conn: &Connection, id: i64, is_liquid: bool) -> Result<()> {
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "UPDATE accounts SET is_liquid = ?1, updated_at = ?2 WHERE id = ?3",
            params![is_liquid as i64, now, id],
        )?;
        Ok(())
    }

    pub fn total_balance(conn: &Connection) -> Result<f64> {
        Ok(conn.query_row(
            "SELECT COALESCE(SUM(balance), 0.0) FROM accounts",
            [],
            |r| r.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::account::AccountType;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_create_and_find() {
        let conn = test_conn();
        let req = NewAccount {
            name: "测试钱包".into(),
            account_type: AccountType::Cash,
            currency: "CNY".into(),
            initial_balance: 100.0,
            is_liquid: true,
        };
        let acc = AccountDao::create(&conn, &req).unwrap();
        assert_eq!(acc.name, "测试钱包");
        assert_eq!(acc.balance, 100.0);

        let found = AccountDao::find_by_id(&conn, acc.id).unwrap();
        assert!(found.is_some());
    }

    #[test]
    fn test_adjust_balance() {
        let conn = test_conn();
        let acc = AccountDao::create(
            &conn,
            &NewAccount {
                name: "钱包".into(),
                account_type: AccountType::Cash,
                currency: "CNY".into(),
                initial_balance: 200.0,
            is_liquid: true,
            },
        )
        .unwrap();

        AccountDao::adjust_balance(&conn, acc.id, -50.0).unwrap();
        let updated = AccountDao::find_by_id(&conn, acc.id).unwrap().unwrap();
        assert!((updated.balance - 150.0).abs() < 1e-9);
    }
}
