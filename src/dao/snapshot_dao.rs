use rusqlite::{params, Connection};

use crate::error::Result;
use crate::models::snapshot::{AccountSnapshot, MonthlyTotal};

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<AccountSnapshot> {
    Ok(AccountSnapshot {
        id: row.get(0)?,
        year: row.get(1)?,
        month: row.get(2)?,
        account_id: row.get(3)?,
        account_name: row.get(4)?,
        account_type: row.get(5)?,
        balance: row.get(6)?,
        note: row.get(7)?,
        created_at: row.get(8)?,
    })
}

pub struct SnapshotDao;

impl SnapshotDao {
    /// 保存/更新某月某账户的余额快照（UPSERT）
    pub fn upsert(
        conn: &Connection,
        year: i32,
        month: u32,
        account_id: i64,
        balance: f64,
        note: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO balance_snapshots (year, month, account_id, balance, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(year, month, account_id) DO UPDATE SET
                 balance    = excluded.balance,
                 note       = excluded.note,
                 created_at = excluded.created_at",
            params![year, month, account_id, balance, note, now],
        )?;
        Ok(())
    }

    /// 查询某月所有账户快照
    pub fn find_month(conn: &Connection, year: i32, month: u32) -> Result<Vec<AccountSnapshot>> {
        let mut stmt = conn.prepare(
            "SELECT bs.id, bs.year, bs.month, bs.account_id,
                    a.name, a.account_type, bs.balance, bs.note, bs.created_at
             FROM balance_snapshots bs
             JOIN accounts a ON bs.account_id = a.id
             WHERE bs.year = ?1 AND bs.month = ?2
             ORDER BY a.name",
        )?;
        let rows = stmt.query_map(params![year, month], map_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// 查询最近 N 个月的总资产（用于趋势展示）
    pub fn monthly_totals(conn: &Connection, limit: i64) -> Result<Vec<MonthlyTotal>> {
        let mut stmt = conn.prepare(
            "SELECT year, month, SUM(balance) as total
             FROM balance_snapshots
             GROUP BY year, month
             ORDER BY year DESC, month DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            Ok(MonthlyTotal {
                year: r.get(0)?,
                month: r.get::<_, u32>(1)?,
                total: r.get(2)?,
            })
        })?;
        let mut result: Vec<MonthlyTotal> = rows.collect::<rusqlite::Result<_>>()?;
        result.reverse(); // 时间正序
        Ok(result)
    }

    /// 查询每个账户最新一次快照的余额（用于月结表单的"上次余额"）
    pub fn latest_per_account(conn: &Connection) -> Result<Vec<(i64, f64, i32, u32)>> {
        let mut stmt = conn.prepare(
            "SELECT bs.account_id, bs.balance, bs.year, bs.month
             FROM balance_snapshots bs
             INNER JOIN (
                 SELECT account_id, MAX(year * 12 + month) AS max_ym
                 FROM balance_snapshots
                 GROUP BY account_id
             ) latest
               ON bs.account_id = latest.account_id
              AND (bs.year * 12 + bs.month) = latest.max_ym",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, u32>(3)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// 检查某月是否已有快照
    pub fn has_snapshot(conn: &Connection, year: i32, month: u32) -> Result<bool> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM balance_snapshots WHERE year = ?1 AND month = ?2",
            params![year, month],
            |r| r.get(0),
        )?;
        Ok(count > 0)
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
        conn
    }

    fn mk_account(conn: &Connection, name: &str) -> i64 {
        AccountDao::create(
            conn,
            &NewAccount {
                name: name.into(),
                account_type: AccountType::Bank,
                currency: "CNY".into(),
                initial_balance: 0.0,
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn test_upsert_and_find() {
        let conn = setup();
        let id = mk_account(&conn, "工资卡");

        SnapshotDao::upsert(&conn, 2024, 1, id, 10000.0, None).unwrap();
        SnapshotDao::upsert(&conn, 2024, 2, id, 12000.0, Some("涨薪了")).unwrap();

        let jan = SnapshotDao::find_month(&conn, 2024, 1).unwrap();
        assert_eq!(jan.len(), 1);
        assert!((jan[0].balance - 10000.0).abs() < 1e-9);

        // UPSERT：更新同月同账户
        SnapshotDao::upsert(&conn, 2024, 1, id, 11000.0, None).unwrap();
        let jan2 = SnapshotDao::find_month(&conn, 2024, 1).unwrap();
        assert!((jan2[0].balance - 11000.0).abs() < 1e-9);
    }

    #[test]
    fn test_monthly_totals() {
        let conn = setup();
        let id1 = mk_account(&conn, "账户A");
        let id2 = mk_account(&conn, "账户B");

        SnapshotDao::upsert(&conn, 2024, 1, id1, 5000.0, None).unwrap();
        SnapshotDao::upsert(&conn, 2024, 1, id2, 3000.0, None).unwrap();
        SnapshotDao::upsert(&conn, 2024, 2, id1, 6000.0, None).unwrap();
        SnapshotDao::upsert(&conn, 2024, 2, id2, 4000.0, None).unwrap();

        let totals = SnapshotDao::monthly_totals(&conn, 12).unwrap();
        assert_eq!(totals.len(), 2);
        assert!((totals[0].total - 8000.0).abs() < 1e-9); // 2024-01
        assert!((totals[1].total - 10000.0).abs() < 1e-9); // 2024-02
    }
}
