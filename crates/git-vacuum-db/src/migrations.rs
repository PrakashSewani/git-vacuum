use crate::connection::ConnectionPool;

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "001_initial_schema",
        sql: include_str!("migrations/001_initial_schema.sql"),
    },
];

pub async fn run_migrations(pool: &ConnectionPool) -> Result<(), String> {
    pool.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    ).await?;

    let current_version: i64 = match pool.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        &[],
        |row| row.get(0),
    ).await {
        Ok(v) => v,
        Err(_) => 0,
    };

    for migration in MIGRATIONS {
        if migration.version > current_version {
            pool.execute_batch(migration.sql).await?;
            log::info!("Applied migration {}: {}", migration.version, migration.name);
        }
    }

    Ok(())
}
