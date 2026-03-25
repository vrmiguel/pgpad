use std::collections::{HashMap, HashSet};

use anyhow::Context;
use tokio_postgres::Client;

use crate::{
    database::types::{ColumnInfo, DatabaseSchema, TableInfo},
    Error,
};

pub async fn get_database_schema(client: &Client) -> Result<DatabaseSchema, Error> {
    let schema_query = r#"
        SELECT 
            t.table_schema,
            t.table_name,
            c.column_name,
            c.data_type,
            c.is_nullable::boolean,
            c.column_default
        FROM 
            information_schema.tables t
        JOIN 
            information_schema.columns c 
            ON t.table_name = c.table_name 
            AND t.table_schema = c.table_schema
        WHERE 
            t.table_type = 'BASE TABLE'
            AND t.table_schema NOT IN ('information_schema', 'pg_catalog', 'pg_toast')
            -- Exclude child partitions: keep only regular tables and partition roots
            AND NOT EXISTS (
                SELECT 1
                FROM pg_inherits i
                JOIN pg_class child ON child.oid = i.inhrelid
                JOIN pg_namespace child_ns ON child_ns.oid = child.relnamespace
                WHERE child.relname = t.table_name
                AND child_ns.nspname = t.table_schema
            )
        ORDER BY 
            t.table_schema, t.table_name, c.ordinal_position
    "#;

    let rows = client
        .query(schema_query, &[])
        .await
        .context("Failed to query database schema")?;

    // Key is (schema, table_name)
    let mut tables_map = HashMap::new();
    let mut schemas_set = HashSet::new();
    let mut unique_columns_set = HashSet::new();

    for row in &rows {
        let schema: &str = row.get(0);
        let table_name: &str = row.get(1);
        let column_name: &str = row.get(2);
        let data_type: &str = row.get(3);
        let is_nullable: bool = row.get(4);
        let default_value: Option<&str> = row.get(5);

        schemas_set.insert(schema);
        unique_columns_set.insert(column_name);

        let table_key = (schema, table_name);

        let table_info = tables_map.entry(table_key).or_insert_with(|| TableInfo {
            name: table_name.to_owned(),
            schema: schema.to_owned(),
            columns: Vec::new(),
        });

        table_info.columns.push(ColumnInfo {
            name: column_name.to_owned(),
            data_type: data_type.to_owned(),
            is_nullable,
            default_value: default_value.map(|s| s.to_owned()),
        });
    }

    let tables = tables_map.into_values().collect();
    let schemas = schemas_set.into_iter().map(ToOwned::to_owned).collect();
    let unique_columns = unique_columns_set
        .into_iter()
        .map(ToOwned::to_owned)
        .collect();

    Ok(DatabaseSchema {
        tables,
        schemas,
        unique_columns,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use anyhow::Context;
    use pgtemp::PgTempDB;

    use super::get_database_schema;

    #[tokio::test]
    async fn excludes_partition_children_from_schema_listing() -> anyhow::Result<()> {
        let db = PgTempDB::async_new().await;
        let (client, conn) = tokio_postgres::connect(&db.connection_uri(), tokio_postgres::NoTls)
            .await
            .context("Failed to connect to temporary postgres")?;

        tokio::task::spawn(async move {
            if let Err(e) = conn.await {
                eprintln!("Connection error: {}", e);
            }
        });

        client
            .batch_execute(
                "
                CREATE TABLE measurements (
                    city_id int NOT NULL,
                    logdate date NOT NULL,
                    peaktemp int,
                    unitsales int
                ) PARTITION BY RANGE (logdate);

                CREATE TABLE measurements_2025_01 PARTITION OF measurements
                    FOR VALUES FROM ('2025-01-01') TO ('2025-02-01');

                CREATE TABLE measurements_2025_02 PARTITION OF measurements
                    FOR VALUES FROM ('2025-02-01') TO ('2025-03-01');
                ",
            )
            .await
            .context("Failed to create partitioned test tables")?;

        let schema = get_database_schema(&client).await?;

        let measurement_tables: HashSet<String> = schema
            .tables
            .into_iter()
            .filter(|table| table.name.starts_with("measurements"))
            .map(|table| table.name)
            .collect();

        assert_eq!(
            measurement_tables,
            HashSet::from([String::from("measurements")])
        );

        Ok(())
    }
}
