use futures_util::TryStreamExt;
use tiberius::Client;

use crate::database::types::{ColumnInfo, DatabaseSchema, TableInfo};
use crate::Error;

pub async fn get_database_schema(
    client: &mut Client<crate::database::mssql::connect::MssqlStream>,
) -> Result<DatabaseSchema, Error> {
    let tables_sql = r#"
        SELECT TABLE_SCHEMA, TABLE_NAME
        FROM INFORMATION_SCHEMA.TABLES
        WHERE TABLE_TYPE IN ('BASE TABLE','VIEW')
        ORDER BY TABLE_SCHEMA, TABLE_NAME
    "#;

    let mut tables = Vec::new();
    let mut schemas = Vec::new();

    let mut tstream = client
        .simple_query(tables_sql)
        .await
        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
    while let Some(item) = tstream
        .try_next()
        .await
        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
    {
        if let tiberius::QueryItem::Row(row) = item {
            let schema: &str = row
                .try_get(0)
                .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
                .unwrap_or("");
            let name: &str = row
                .try_get(1)
                .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
                .unwrap_or("");
            if !schemas.iter().any(|s| s == schema) {
                schemas.push(schema.to_string());
            }
            tables.push((schema.to_string(), name.to_string()));
        }
    }
    drop(tstream);

    let mut table_infos = Vec::new();
    for (schema, name) in tables {
        // Fall back to simple_query without params for compatibility
        let cols_sql_plain = format!(
            "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' ORDER BY ORDINAL_POSITION",
            schema.replace("'", "''"),
            name.replace("'", "''")
        );
        let mut cstream = client
            .simple_query(&cols_sql_plain)
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
        let mut columns = Vec::new();
        while let Some(item) = cstream
            .try_next()
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
        {
            if let tiberius::QueryItem::Row(row) = item {
                let col_name: &str = row
                    .try_get(0)
                    .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
                    .unwrap_or("");
                let data_type: &str = row
                    .try_get(1)
                    .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
                    .unwrap_or("");
                let is_nullable: &str = row
                    .try_get(2)
                    .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
                    .unwrap_or("YES");
                let default_value: Option<&str> = row
                    .try_get(3)
                    .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                columns.push(ColumnInfo {
                    name: col_name.to_string(),
                    data_type: data_type.to_string(),
                    is_nullable: is_nullable.eq_ignore_ascii_case("YES"),
                    default_value: default_value.map(|s| s.to_string()),
                });
            }
        }
        table_infos.push(TableInfo {
            name: name.clone(),
            schema: schema.clone(),
            columns,
        });
    }

    let mut unique_columns = Vec::new();
    for t in &table_infos {
        for c in &t.columns {
            if !unique_columns.iter().any(|x| x == &c.name) {
                unique_columns.push(c.name.clone());
            }
        }
    }

    // Enrich unique columns from key constraints
    {
        let mut kstream = client
            .simple_query(
                "SELECT KCU.COLUMN_NAME FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS TC JOIN INFORMATION_SCHEMA.KEY_COLUMN_USAGE KCU ON TC.CONSTRAINT_NAME = KCU.CONSTRAINT_NAME AND TC.TABLE_SCHEMA = KCU.TABLE_SCHEMA AND TC.TABLE_NAME = KCU.TABLE_NAME WHERE TC.CONSTRAINT_TYPE IN ('PRIMARY KEY','UNIQUE','FOREIGN KEY')"
            )
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
        while let Some(item) = kstream
            .try_next()
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
        {
            if let tiberius::QueryItem::Row(row) = item {
                if let Ok(Some(col)) = row.try_get::<&str, _>(0) {
                    if !unique_columns.iter().any(|x| x == col) {
                        unique_columns.push(col.to_string());
                    }
                }
            }
        }
    }

    // Enrich unique columns from indexes
    // previous stream is dropped at end of scope
    let idx_sql = r#"
        SELECT c.name AS column_name
        FROM sys.indexes i
        JOIN sys.index_columns ic ON i.object_id = ic.object_id AND i.index_id = ic.index_id
        JOIN sys.columns c ON c.object_id = i.object_id AND c.column_id = ic.column_id
    "#;
    let mut istream = client
        .simple_query(idx_sql)
        .await
        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
    while let Some(item) = istream
        .try_next()
        .await
        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
    {
        if let tiberius::QueryItem::Row(row) = item {
            if let Ok(Some(col)) = row.try_get::<&str, _>(0) {
                if !unique_columns.iter().any(|x| x == col) {
                    unique_columns.push(col.to_string());
                }
            }
        }
    }

    Ok(DatabaseSchema {
        tables: table_infos,
        schemas,
        unique_columns,
    })
}
