use rusqlite::{params, Connection, OptionalExtension};
use std::str::FromStr;

use crate::error::{AppError, Result};
use crate::models::category::{Category, CategoryType, NewCategory};

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Category> {
    let type_str: String = row.get(2)?;
    Ok(Category {
        id: row.get(0)?,
        name: row.get(1)?,
        category_type: CategoryType::from_str(&type_str).unwrap_or(CategoryType::Expense),
        icon: row.get(3)?,
        parent_id: row.get(4)?,
    })
}

pub struct CategoryDao;

impl CategoryDao {
    pub fn create(conn: &Connection, req: &NewCategory) -> Result<Category> {
        conn.execute(
            "INSERT INTO categories (name, category_type, icon, parent_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![req.name, req.category_type.to_string(), req.icon, req.parent_id],
        )?;
        let id = conn.last_insert_rowid();
        Self::find_by_id(conn, id)?.ok_or_else(|| AppError::NotFound("Category".into()))
    }

    pub fn find_by_id(conn: &Connection, id: i64) -> Result<Option<Category>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, category_type, icon, parent_id FROM categories WHERE id = ?1",
        )?;
        Ok(stmt.query_row(params![id], map_row).optional()?)
    }

    pub fn find_all(conn: &Connection) -> Result<Vec<Category>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, category_type, icon, parent_id
             FROM categories ORDER BY category_type, name",
        )?;
        let rows = stmt.query_map([], map_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    #[allow(dead_code)]
    pub fn find_by_type(conn: &Connection, cat_type: &CategoryType) -> Result<Vec<Category>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, category_type, icon, parent_id
             FROM categories WHERE category_type = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![cat_type.to_string()], map_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    #[allow(dead_code)]
    pub fn delete(conn: &Connection, id: i64) -> Result<()> {
        conn.execute("DELETE FROM categories WHERE id = ?1", params![id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_create_and_find_all() {
        let conn = test_conn();
        let req = NewCategory {
            name: "测试分类".into(),
            category_type: CategoryType::Expense,
            icon: Some("🧪".into()),
            parent_id: None,
        };
        let cat = CategoryDao::create(&conn, &req).unwrap();
        assert_eq!(cat.name, "测试分类");

        let all = CategoryDao::find_all(&conn).unwrap();
        assert!(!all.is_empty());
    }

    #[test]
    fn test_find_by_type() {
        let conn = test_conn();
        crate::db::migrations::seed_default_data(&conn).unwrap();

        let expense_cats = CategoryDao::find_by_type(&conn, &CategoryType::Expense).unwrap();
        let income_cats = CategoryDao::find_by_type(&conn, &CategoryType::Income).unwrap();
        assert!(!expense_cats.is_empty());
        assert!(!income_cats.is_empty());
        assert!(expense_cats.iter().all(|c| c.category_type == CategoryType::Expense));
    }
}
