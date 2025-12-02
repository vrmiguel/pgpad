use std::time::Instant;

use duckdb::Connection;
use std::env;

use crate::{
    database::{
        duckdb::row_writer::RowWriter, parser::ParsedStatement, types::ExecSender, QueryExecEvent,
    },
    utils::serialize_as_json_array,
    Error,
};

pub fn execute_query(
    client: &Connection,
    stmt: ParsedStatement,
    sender: &ExecSender,
    settings: Option<&crate::database::types::OracleSettings>,
) -> Result<(), Error> {
    let start = std::time::Instant::now();

    if stmt.returns_values {
        execute_query_with_results(client, &stmt.statement, sender, start, settings)?;
    } else {
        execute_modification_query(client, &stmt.statement, sender, start)?;
    }

    Ok(())
}

pub fn execute_query_with_params(
    client: &Connection,
    stmt: ParsedStatement,
    sender: &ExecSender,
    params: serde_json::Map<String, serde_json::Value>,
    settings: Option<&crate::database::types::OracleSettings>,
) -> Result<(), Error> {
    fn find_max_positional(sql: &str) -> usize {
        let bytes = sql.as_bytes();
        let mut i = 0usize;
        let mut max_idx = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'$' {
                i += 1;
                let mut n = 0usize;
                let mut has = false;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    has = true;
                    n = n * 10 + (bytes[i] - b'0') as usize;
                    i += 1;
                }
                if has && n > max_idx {
                    max_idx = n;
                }
            } else {
                i += 1;
            }
        }
        max_idx
    }

    fn collect_named(sql: &str) -> Vec<String> {
        let mut names = Vec::new();
        let bytes = sql.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'$' {
                i += 1;
                let start = i;
                if i < bytes.len() && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
                    i += 1;
                    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
                    {
                        i += 1;
                    }
                    let name = &sql[start..i];
                    if !name.chars().all(|c| c.is_ascii_digit()) {
                        names.push(name.to_string());
                    }
                }
            } else {
                i += 1;
            }
        }
        names
    }

    enum ParamVal {
        I64(i64),
        F64(f64),
        Bool(bool),
        Str(String),
        StrOpt(Option<String>),
        Json(String),
    }
    fn map_param(v: &serde_json::Value) -> ParamVal {
        match v {
            serde_json::Value::Null => ParamVal::StrOpt(None),
            serde_json::Value::Bool(b) => ParamVal::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    ParamVal::I64(i)
                } else if let Some(f) = n.as_f64() {
                    ParamVal::F64(f)
                } else {
                    ParamVal::Str(n.to_string())
                }
            }
            serde_json::Value::String(s) => ParamVal::Str(s.clone()),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                ParamVal::Json(v.to_string())
            }
        }
    }
    fn get_by_keys(m: &serde_json::Map<String, serde_json::Value>, key: &str) -> ParamVal {
        m.get(key).map(map_param).unwrap_or(ParamVal::StrOpt(None))
    }

    let mut owned: Vec<Box<dyn duckdb::ToSql>> = Vec::new();
    let mut refs: Vec<&dyn duckdb::ToSql> = Vec::new();
    let mut json_flags: Vec<bool> = Vec::new();

    let positional_max = find_max_positional(&stmt.statement);
    let named = collect_named(&stmt.statement);
    let q_count = stmt
        .statement
        .as_bytes()
        .iter()
        .filter(|&&b| b == b'?')
        .count();

    if positional_max > 0 {
        for i in 1..=positional_max {
            let k1 = i.to_string();
            let k2 = format!("${}", i);
            let k3 = format!("P{}", i);
            let k4 = format!("p{}", i);
            let pv = if let Some(v) = params.get(&k1) {
                map_param(v)
            } else if let Some(v) = params.get(&k2) {
                map_param(v)
            } else if let Some(v) = params.get(&k3) {
                map_param(v)
            } else if let Some(v) = params.get(&k4) {
                map_param(v)
            } else {
                ParamVal::StrOpt(None)
            };
            match pv {
                ParamVal::I64(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::F64(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::Bool(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::Str(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::StrOpt(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::Json(s) => {
                    owned.push(Box::new(s));
                    json_flags.push(true);
                }
            }
        }
    } else if !named.is_empty() {
        for name in &named {
            let pv = get_by_keys(&params, name);
            match pv {
                ParamVal::I64(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::F64(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::Bool(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::Str(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::StrOpt(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::Json(s) => {
                    owned.push(Box::new(s));
                    json_flags.push(true);
                }
            }
        }
    } else if q_count > 0 {
        for i in 1..=q_count {
            let k1 = i.to_string();
            let k2 = format!("P{}", i);
            let k3 = format!("p{}", i);
            let pv = if let Some(v) = params.get(&k1) {
                map_param(v)
            } else if let Some(v) = params.get(&k2) {
                map_param(v)
            } else if let Some(v) = params.get(&k3) {
                map_param(v)
            } else {
                ParamVal::StrOpt(None)
            };
            match pv {
                ParamVal::I64(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::F64(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::Bool(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::Str(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::StrOpt(v) => {
                    owned.push(Box::new(v));
                    json_flags.push(false);
                }
                ParamVal::Json(s) => {
                    owned.push(Box::new(s));
                    json_flags.push(true);
                }
            }
        }
    }

    for o in owned.iter() {
        refs.push(o.as_ref());
    }

    let mut sql = stmt.statement.clone();
    if positional_max > 0 {
        for (i, is_json) in json_flags.iter().enumerate() {
            if *is_json {
                let token = format!("${}", i + 1);
                let wrapped = format!("CAST(${} AS JSON)", i + 1);
                sql = sql.replace(&token, &wrapped);
            }
        }
    } else if !named.is_empty() {
        for (idx, name) in collect_named(&stmt.statement).into_iter().enumerate() {
            if idx < json_flags.len() && json_flags[idx] {
                let token = format!("${}", name);
                let wrapped = format!("CAST(${} AS JSON)", name);
                sql = sql.replace(&token, &wrapped);
            }
        }
    } else if q_count > 0 {
        let mut rebuilt = String::with_capacity(sql.len() + 32);
        let mut qi = 0usize;
        for ch in sql.chars() {
            if ch == '?' {
                let is_json = json_flags.get(qi).copied().unwrap_or(false);
                if is_json {
                    rebuilt.push_str("CAST(? AS JSON)");
                } else {
                    rebuilt.push('?');
                }
                qi += 1;
            } else {
                rebuilt.push(ch);
            }
        }
        sql = rebuilt;
    }

    if stmt.returns_values {
        execute_query_with_results_params(client, &sql, sender, settings, &refs)
    } else {
        execute_modification_query_params(client, &sql, sender, &refs)
    }
}

fn execute_query_with_results(
    client: &Connection,
    query: &str,
    sender: &ExecSender,
    started_at: Instant,
    settings: Option<&crate::database::types::OracleSettings>,
) -> Result<(), Error> {
    log::info!("Starting DuckDB query: {}", query);

    match client.prepare(query) {
        Ok(mut stmt) => {
            let mut rows = stmt.query([])?;
            let column_count = rows.as_ref().unwrap().column_count();

            let mut column_names = Vec::with_capacity(column_count);
            for i in 0..column_count {
                column_names.push(rows.as_ref().unwrap().column_name(i)?.to_string());
            }
            let columns = serialize_as_json_array(column_names.iter().map(|s| s.as_str()))?;

            sender.send(QueryExecEvent::TypesResolved { columns })?;

            let mut total_rows = 0;
            let batch_size = settings
                .and_then(|s| s.batch_size)
                .or_else(|| {
                    env::var("PGPAD_BATCH_SIZE")
                        .ok()
                        .and_then(|v| v.parse::<usize>().ok())
                        .filter(|&n| n > 0)
                })
                .unwrap_or(50);

            // Retrieve logical types when available to assist formatting
            let mut column_decltypes: Vec<Option<String>> = Vec::with_capacity(column_count);
            for i in 0..column_count {
                // Best-effort logical type retrieval
                let ty = Some(format!("{:?}", rows.as_ref().unwrap().column_type(i)));
                column_decltypes.push(ty);
            }
            // If column_decltypes are all None but we can infer from first row, leave to RowWriter's per-value handling
            let mut writer = match settings {
                Some(s) => RowWriter::with_settings(column_decltypes, Some(s)),
                None => RowWriter::new(column_decltypes),
            };

            while let Some(row) = rows.next()? {
                writer.add_row(row)?;
                total_rows += 1;
                if writer.len() >= batch_size {
                    sender.send(QueryExecEvent::Page {
                        page_amount: writer.len(),
                        page: writer.finish(),
                    })?;
                }
            }

            if !writer.is_empty() {
                sender.send(QueryExecEvent::Page {
                    page_amount: writer.len(),
                    page: writer.finish(),
                })?;
            }

            let duration = started_at.elapsed().as_millis() as u64;
            log::info!(
                "DuckDB query completed: {} rows in {}ms",
                total_rows,
                duration
            );

            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: None,
            })?;
            Ok(())
        }
        Err(e) => {
            log::error!("DuckDB statement preparation failed: {:?}", e);
            let error_msg = format!("Query failed: {}", e);
            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: Some(error_msg.clone()),
            })?;
            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

fn execute_modification_query(
    client: &Connection,
    query: &str,
    sender: &ExecSender,
    started_at: Instant,
) -> Result<(), Error> {
    log::info!("Executing DuckDB modification query: {}", query);

    match client.prepare(query) {
        Ok(mut stmt) => match stmt.execute([]) {
            Ok(rows_affected) => {
                sender.send(QueryExecEvent::Finished {
                    elapsed_ms: started_at.elapsed().as_millis() as u64,
                    affected_rows: rows_affected,
                    error: None,
                })?;
                Ok(())
            }
            Err(e) => {
                log::error!("DuckDB modification query failed: {:?}", e);
                let error_msg = format!("Query failed: {}", e);
                sender.send(QueryExecEvent::Finished {
                    elapsed_ms: started_at.elapsed().as_millis() as u64,
                    affected_rows: 0,
                    error: Some(error_msg.clone()),
                })?;
                Err(Error::Any(anyhow::anyhow!(error_msg)))
            }
        },
        Err(e) => {
            log::error!("DuckDB statement preparation failed: {:?}", e);
            let error_msg = format!("Query failed: {}", e);
            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: Some(error_msg.clone()),
            })?;
            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

fn execute_query_with_results_params(
    client: &Connection,
    query: &str,
    sender: &ExecSender,
    settings: Option<&crate::database::types::OracleSettings>,
    params: &[&dyn duckdb::ToSql],
) -> Result<(), Error> {
    let started_at = std::time::Instant::now();
    log::info!("Starting DuckDB prepared query: {}", query);
    match client.prepare(query) {
        Ok(mut stmt) => {
            let mut rows = stmt.query(params)?;
            let column_count = rows.as_ref().unwrap().column_count();
            let mut column_names = Vec::with_capacity(column_count);
            for i in 0..column_count {
                column_names.push(rows.as_ref().unwrap().column_name(i)?.to_string());
            }
            let columns = serialize_as_json_array(column_names.iter().map(|s| s.as_str()))?;
            sender.send(QueryExecEvent::TypesResolved { columns })?;
            let mut _total_rows = 0;
            let batch_size = settings
                .and_then(|s| s.batch_size)
                .or_else(|| {
                    env::var("PGPAD_BATCH_SIZE")
                        .ok()
                        .and_then(|v| v.parse::<usize>().ok())
                        .filter(|&n| n > 0)
                })
                .unwrap_or(50);
            let mut column_decltypes: Vec<Option<String>> = Vec::with_capacity(column_count);
            for i in 0..column_count {
                let ty = Some(format!("{:?}", rows.as_ref().unwrap().column_type(i)));
                column_decltypes.push(ty);
            }
            let mut writer = match settings {
                Some(s) => RowWriter::with_settings(column_decltypes, Some(s)),
                None => RowWriter::new(column_decltypes),
            };
            while let Some(row) = rows.next()? {
                writer.add_row(row)?;
                _total_rows += 1;
                if writer.len() >= batch_size {
                    sender.send(QueryExecEvent::Page {
                        page_amount: writer.len(),
                        page: writer.finish(),
                    })?;
                }
            }
            if !writer.is_empty() {
                sender.send(QueryExecEvent::Page {
                    page_amount: writer.len(),
                    page: writer.finish(),
                })?;
            }
            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: None,
            })?;
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Query failed: {}", e);
            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: Some(error_msg.clone()),
            })?;
            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

fn execute_modification_query_params(
    client: &Connection,
    query: &str,
    sender: &ExecSender,
    params: &[&dyn duckdb::ToSql],
) -> Result<(), Error> {
    log::info!("Executing DuckDB prepared modification query: {}", query);
    let started_at = std::time::Instant::now();
    match client.prepare(query) {
        Ok(mut stmt) => match stmt.execute(params) {
            Ok(rows_affected) => {
                sender.send(QueryExecEvent::Finished {
                    elapsed_ms: started_at.elapsed().as_millis() as u64,
                    affected_rows: rows_affected,
                    error: None,
                })?;
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Query failed: {}", e);
                sender.send(QueryExecEvent::Finished {
                    elapsed_ms: started_at.elapsed().as_millis() as u64,
                    affected_rows: 0,
                    error: Some(error_msg.clone()),
                })?;
                Err(Error::Any(anyhow::anyhow!(error_msg)))
            }
        },
        Err(e) => {
            let error_msg = format!("Query failed: {}", e);
            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: Some(error_msg.clone()),
            })?;
            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::execute_query;
    use crate::database::{duckdb::parser::parse_statements, types::channel, QueryExecEvent};
    use duckdb::Connection;
    use std::sync::{Arc, Mutex};

    async fn run_query(
        conn: Arc<Mutex<Connection>>,
        query: &str,
    ) -> anyhow::Result<Vec<QueryExecEvent>> {
        let mut parsed_stmt = parse_statements(query).unwrap();
        assert_eq!(parsed_stmt.len(), 1);
        assert!(parsed_stmt[0].returns_values);
        let stmt = parsed_stmt.pop().unwrap();

        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            execute_query(&conn, stmt, &sender, None).unwrap();
        });

        let mut events = Vec::new();
        while let Some(event) = recv.recv().await {
            events.push(event);
        }
        Ok(events)
    }

    async fn run_modification_query(
        conn: Arc<Mutex<Connection>>,
        query: &str,
    ) -> anyhow::Result<usize> {
        let mut parsed_stmt = parse_statements(query).unwrap();
        assert_eq!(parsed_stmt.len(), 1);
        assert!(!parsed_stmt[0].returns_values);
        let stmt = parsed_stmt.pop().unwrap();

        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            execute_query(&conn, stmt, &sender, None).unwrap();
        });

        let event = recv.recv().await.ok_or(anyhow::anyhow!("Channel closed"))?;
        match event {
            QueryExecEvent::Finished {
                affected_rows,
                error,
                ..
            } => {
                assert!(error.is_none());
                Ok(affected_rows)
            }
            _ => anyhow::bail!("Expected Finished"),
        }
    }

    #[tokio::test]
    async fn test_duckdb_pagination() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory().unwrap();
        std::env::set_var("PGPAD_BATCH_SIZE", "20");
        let conn = Arc::new(Mutex::new(conn));
        let query = "WITH RECURSIVE t(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM t WHERE x < 45) SELECT * FROM t";
        let mut events = run_query(conn.clone(), query).await?.into_iter();
        let types = events.next().unwrap();
        assert!(matches!(types, QueryExecEvent::TypesResolved { .. }));
        let page1 = events.next().unwrap();
        match page1 {
            QueryExecEvent::Page { page_amount, .. } => assert_eq!(page_amount, 20),
            _ => anyhow::bail!("Expected Page 1"),
        };
        let page2 = events.next().unwrap();
        match page2 {
            QueryExecEvent::Page { page_amount, .. } => assert_eq!(page_amount, 20),
            _ => anyhow::bail!("Expected Page 2"),
        };
        let page3 = events.next().unwrap();
        match page3 {
            QueryExecEvent::Page { page_amount, .. } => assert_eq!(page_amount, 5),
            _ => anyhow::bail!("Expected Page 3"),
        };
        Ok(())
    }

    #[tokio::test]
    async fn test_duckdb_modification() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory().unwrap();
        let conn = Arc::new(Mutex::new(conn));
        let _ =
            run_modification_query(conn.clone(), "CREATE TABLE users(id INT, name TEXT)").await?;
        let inserted = run_modification_query(
            conn.clone(),
            "INSERT INTO users VALUES (1, 'Alice'),(2,'Bob')",
        )
        .await?;
        assert_eq!(inserted, 2);
        let updated =
            run_modification_query(conn.clone(), "UPDATE users SET name='Alice2' WHERE id=1")
                .await?;
        assert_eq!(updated, 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_duckdb_float_specials() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory().unwrap();
        let conn = Arc::new(Mutex::new(conn));
        let query = "SELECT CAST('NaN' AS DOUBLE) AS nan_val, CAST('Infinity' AS DOUBLE) AS inf_val, CAST('-Infinity' AS DOUBLE) AS neg_inf_val, 0.0::DOUBLE AS zero_val";
        let mut events = run_query(conn.clone(), query).await?.into_iter();
        let _types = events.next().unwrap();
        let page = events.next().unwrap();
        match page {
            QueryExecEvent::Page { page, .. } => {
                let v: serde_json::Value = serde_json::from_str(page.get())?;
                assert_eq!(v, serde_json::json!([["NaN", "inf", "-inf", 0.0]]));
            }
            _ => anyhow::bail!("Expected Page"),
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_duckdb_temporal_and_decimal() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory().unwrap();
        let conn = Arc::new(Mutex::new(conn));
        let query = "SELECT TIMESTAMP '1992-03-22 01:02:03' AS ts, DATE '1992-03-22' AS d, TIME '01:02:03.000004' AS t, INTERVAL '1 year 2 months 3 days 04:05:06.000007' AS iv, CAST('123.456' AS DECIMAL(18,3)) AS dec";
        let mut events = run_query(conn.clone(), query).await?.into_iter();
        let _types = events.next().unwrap();
        let page = events.next().unwrap();
        match page {
            QueryExecEvent::Page { page, .. } => {
                let v: serde_json::Value = serde_json::from_str(page.get())?;
                let row = &v[0];
                assert!(row[0].as_str().unwrap().starts_with("1992-03-22 01:02:03"));
                assert_eq!(row[1], "1992-03-22");
                assert_eq!(row[2], "01:02:03.000004");
                assert_eq!(row[3], "1 year 2 mons 3 days 04:05:06.000007");
                assert_eq!(row[4], 123.456);
            }
            _ => anyhow::bail!("Expected Page"),
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_duckdb_prepared_params() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory().unwrap();
        let conn = Arc::new(Mutex::new(conn));
        let mut parsed = parse_statements("SELECT $1 AS x, $2 AS y").unwrap();
        let stmt = parsed.pop().unwrap();
        let (sender, mut recv) = channel();
        let mut params = serde_json::Map::new();
        params.insert(
            "1".into(),
            serde_json::Value::Number(serde_json::Number::from(7)),
        );
        params.insert("2".into(), serde_json::Value::String("ok".into()));
        tokio::task::spawn_blocking({
            let conn = conn.clone();
            move || {
                let conn = conn.lock().unwrap();
                super::execute_query_with_params(&conn, stmt, &sender, params, None).unwrap();
            }
        });
        let _ = recv.recv().await.unwrap();
        let page = recv.recv().await.unwrap();
        match page {
            QueryExecEvent::Page { page, .. } => {
                let v: serde_json::Value = serde_json::from_str(page.get())?;
                assert_eq!(v, serde_json::json!([[7, "ok"]]));
            }
            _ => anyhow::bail!("Expected Page"),
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_duckdb_cancel_and_reconnect_flow() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory().unwrap();
        let conn = Arc::new(Mutex::new(conn));
        let mgr = crate::database::stmt_manager::StatementManager::new();
        let client = crate::database::types::DatabaseClient::DuckDB {
            connection: conn.clone(),
        };
        let ids = mgr.submit_query(client, "WITH RECURSIVE t(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM t WHERE x < 1000000) SELECT * FROM t").unwrap();
        mgr.cancel_query(ids[0]).unwrap();
        let status = mgr.get_query_status(ids[0]).unwrap();
        assert!(matches!(status, crate::database::types::QueryStatus::Error));
        let conn2 = Connection::open_in_memory().unwrap();
        let mut stmt = conn2.prepare("SELECT 1")?;
        let mut rows = stmt.query([])?;
        let row = rows.next()?.unwrap();
        let v: i32 = row.get(0)?;
        assert_eq!(v, 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_duckdb_schema_pagination_tables() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE a(id INT); CREATE TABLE b(id INT);")?;
        let mut stmt = conn.prepare("SELECT table_schema, table_name FROM information_schema.tables ORDER BY 1,2 LIMIT ? OFFSET ?")?;
        let mut rows = stmt.query(duckdb::params![1_i64, 0_i64])?;
        let mut cnt = 0;
        while let Some(_r) = rows.next()? {
            cnt += 1;
        }
        assert!(cnt <= 1);
        Ok(())
    }
}
