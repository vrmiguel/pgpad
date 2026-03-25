use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use rusqlite::Connection;

use crate::{
    database::types::{ColumnInfo, DatabaseSchema, TableInfo},
    Error,
};

pub async fn get_database_schema(conn: Arc<Mutex<Connection>>) -> Result<DatabaseSchema, Error> {
    tauri::async_runtime::spawn_blocking(move || {
        let conn = conn.lock().unwrap();

        let mut tables_stmt = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        )?;
        let table_names: Vec<String> = tables_stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut tables = Vec::new();
        let mut unique_columns_set = HashSet::new();

        for table_name in table_names {
            let pragma_query = format!("PRAGMA table_info('{}')", table_name);
            let mut col_stmt = conn
                .prepare(&pragma_query)
                .context("Failed to prepare PRAGMA table_info query")?;

            let col_rows = col_stmt.query_map([], |row| {
                let column_name: String = row.get(1)?;
                let data_type: String = row.get(2)?;
                let not_null: bool = row.get::<_, i32>(3)? != 0;
                let default_value: Option<String> = row.get(4)?;

                Ok((column_name, data_type, !not_null, default_value)) // !not_null = is_nullable
            })?;

            let mut columns = Vec::new();
            for col_result in col_rows {
                let (column_name, data_type, is_nullable, default_value) = col_result?;

                unique_columns_set.insert(column_name.clone());

                columns.push(ColumnInfo {
                    name: column_name,
                    data_type,
                    is_nullable,
                    default_value,
                });
            }

            tables.push(TableInfo {
                name: table_name,
                schema: String::new(),
                columns,
            });
        }

        let unique_columns = unique_columns_set.into_iter().collect();

        Ok(DatabaseSchema {
            tables,
            schemas: vec![],
            unique_columns,
        }) as Result<_, Error>
    })
    .await?
}
