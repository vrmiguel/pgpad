use std::{collections::HashSet, sync::{Arc, Mutex}};

use duckdb::Connection;

use crate::{
    database::types::{ColumnInfo, DatabaseSchema, TableInfo},
    Error,
};

pub async fn get_database_schema(conn: Arc<Mutex<Connection>>) -> Result<DatabaseSchema, Error> {
    tauri::async_runtime::spawn_blocking(move || {
        let conn = conn.lock().map_err(|e| Error::Any(anyhow::anyhow!("Mutex poisoned: {}", e)))?;

        let mut tables_stmt = conn.prepare(
            "SELECT table_schema, table_name FROM information_schema.tables WHERE table_type IN ('BASE TABLE','VIEW')",
        )?;

        let mut rows = tables_stmt.query([])?;
        let mut tables = Vec::new();
        let mut unique_columns_set = HashSet::new();
        let mut schemas_set = HashSet::new();

        while let Some(row) = rows.next()? {
            let schema: String = row.get(0)?;
            let table_name: String = row.get(1)?;

            schemas_set.insert(schema.clone());

            let mut col_stmt = conn.prepare(
                "SELECT column_name, data_type, is_nullable, column_default FROM information_schema.columns WHERE table_schema = ? AND table_name = ? ORDER BY ordinal_position",
            )?;
            let mut col_rows = col_stmt.query(duckdb::params![&schema, &table_name])?;

            let mut columns = Vec::new();
            while let Some(col_row) = col_rows.next()? {
                let column_name: String = col_row.get(0)?;
                let data_type: String = col_row.get(1)?;
                let is_nullable_str: String = col_row.get(2)?;
                let is_nullable = is_nullable_str.eq_ignore_ascii_case("YES");
                let default_value: Option<String> = col_row.get(3)?;

                unique_columns_set.insert(column_name.clone());

                columns.push(ColumnInfo { name: column_name, data_type, is_nullable, default_value });
            }

            tables.push(TableInfo { name: table_name, schema, columns });
        }

        let unique_columns = unique_columns_set.into_iter().collect();
        let mut schemas: Vec<String> = schemas_set.into_iter().collect();
        schemas.sort();
        Ok(DatabaseSchema { tables, schemas, unique_columns }) as Result<_, Error>
    }).await?
}

#[cfg(test)]
mod tests {
    use super::get_database_schema;
    use duckdb::Connection;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn test_schema_introspection() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("CREATE SCHEMA IF NOT EXISTS test; CREATE TABLE test.users(id INT, name TEXT);")?;
        let schema = get_database_schema(Arc::new(Mutex::new(conn))).await?;
        assert!(schema.schemas.iter().any(|s| s == "test"));
        assert!(schema.tables.iter().any(|t| t.schema == "test" && t.name == "users"));
        Ok(())
    }
}
