use std::path::PathBuf;

use anyhow::Context;
use rusqlite::Connection;

use crate::Result;

pub struct Storage {}

impl Storage {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let mut conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open SQLite database at {}", db_path.display()))?;
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = FULL;
            PRAGMA busy_timeout = 30000;
            PRAGMA case_sensitive_like = ON;
            PRAGMA extended_result_codes = ON;
            ",
        )
        .context("Failed to execute database initialization SQL")?;

        Ok(Self {})
    }
}
