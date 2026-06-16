use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct ConnectionPool {
    conn: Arc<Mutex<Connection>>,
    db_path: PathBuf,
}

impl ConnectionPool {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create db dir: {}", e))?;
        }

        let conn = Connection::open(path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;"
        ).map_err(|e| format!("Failed to set pragmas: {}", e))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: path.to_path_buf(),
        })
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub async fn execute(&self, sql: &str, params: &[Box<dyn rusqlite::types::ToSql + Send + Sync>]) -> Result<usize, String> {
        let conn = self.conn.lock().await;
        conn.execute(sql, rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())))
            .map_err(|e| format!("SQL error: {}", e))
    }

    pub async fn execute_batch(&self, sql: &str) -> Result<(), String> {
        let conn = self.conn.lock().await;
        conn.execute_batch(sql).map_err(|e| format!("Batch error: {}", e))
    }

    pub async fn query_row<T, F>(&self, sql: &str, params: &[Box<dyn rusqlite::types::ToSql + Send + Sync>], f: F) -> Result<T, String>
    where
        F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.conn.lock().await;
        conn.query_row(sql, rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())), f)
            .map_err(|e| format!("Query error: {}", e))
    }

    pub async fn query_map<T, F>(
        &self,
        sql: &str,
        params: &[Box<dyn rusqlite::types::ToSql + Send + Sync>],
        f: F,
    ) -> Result<Vec<T>, String>
    where
        F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T> + Send,
        T: Send + 'static,
    {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(sql).map_err(|e| format!("Prepare error: {}", e))?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())), f)
            .map_err(|e| format!("Query error: {}", e))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
    }

    pub async fn last_insert_rowid(&self) -> Result<i64, String> {
        let conn = self.conn.lock().await;
        Ok(conn.last_insert_rowid())
    }
}
