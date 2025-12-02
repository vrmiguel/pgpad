use std::time::Instant;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use futures_util::TryStreamExt;
use tiberius::{Client, Query, QueryItem};
use uuid::Uuid;

use crate::database::mssql::row_writer::RowWriter;
use crate::database::parser::ParsedStatement;
use crate::database::types::ExecSender;
use crate::Error;

fn extract_inner_sql(sql: &str) -> String {
    let s = sql.trim_start();
    let up = s.to_uppercase();
    if !up.starts_with("EXPLAIN") {
        return sql.to_string();
    }
    for kw in ["SELECT", "UPDATE", "INSERT", "DELETE", "MERGE"] {
        if let Some(pos) = up.find(kw) {
            return s[pos..].to_string();
        }
    }
    s.strip_prefix(|c: char| c.is_whitespace())
        .unwrap_or(s)
        .to_string()
}

fn batch_size() -> usize {
    std::env::var("PGPAD_BATCH_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(50)
}

fn env_truthy(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
}

pub async fn execute_query(
    client: &mut Client<crate::database::mssql::connect::MssqlStream>,
    stmt: ParsedStatement,
    sender: &ExecSender,
    settings: Option<&crate::database::types::OracleSettings>,
) -> Result<(), Error> {
    let now = Instant::now();
    let sql = stmt.statement;
    let mut affected_rows = 0usize;

    // Basic explain plan integration via SHOWPLAN_XML
    if stmt.explain_plan {
        client
            .simple_query("SET SHOWPLAN_XML ON")
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(format!("Enable SHOWPLAN failed: {}", e))))?;

        let inner_sql = extract_inner_sql(&sql);
        let mut stream = client
            .simple_query(inner_sql)
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(format!("Explain failed: {}", e))))?;

        let mut writer = RowWriter::with_settings(settings);
        let mut columns_sent = false;
        while let Some(item) = stream
            .try_next()
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
        {
            match item {
                tiberius::QueryItem::Metadata(meta) => {
                    if !columns_sent {
                        let cols: Vec<String> = meta
                            .columns()
                            .iter()
                            .map(|c| c.name().to_string())
                            .collect();
                        let cols_json = serde_json::to_string(&cols)
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        sender
                            .send(crate::database::types::QueryExecEvent::TypesResolved {
                                columns: serde_json::value::RawValue::from_string(cols_json)
                                    .unwrap(),
                            })
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        columns_sent = true;
                    }
                }
                tiberius::QueryItem::Row(row) => {
                    writer
                        .add_row(&row)
                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let batch = batch_size();
                    if writer.len() >= batch {
                        let page = writer.finish();
                        sender
                            .send(crate::database::types::QueryExecEvent::Page {
                                page_amount: 0,
                                page,
                            })
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    }
                }
            }
        }

        drop(stream);

        if !writer.is_empty() {
            let page = writer.finish();
            sender
                .send(crate::database::types::QueryExecEvent::Page {
                    page_amount: 0,
                    page,
                })
                .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
        }

        client
            .simple_query("SET SHOWPLAN_XML OFF")
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(format!("Disable SHOWPLAN failed: {}", e))))?;
    } else if stmt.returns_values {
        let mut stream = client.simple_query(sql).await.map_err(|e| {
            Error::Any(anyhow::anyhow!(map_mssql_error(&format!(
                "Query failed to start: {}",
                e
            ))))
        })?;

        let mut writer = RowWriter::with_settings(settings);
        let mut columns_sent = false;
        let mut pending_pages: Vec<Box<serde_json::value::RawValue>> = Vec::new();

        while let Some(item) = stream
            .try_next()
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
        {
            match item {
                QueryItem::Metadata(meta) => {
                    if !columns_sent {
                        let cols: Vec<String> = meta
                            .columns()
                            .iter()
                            .map(|c| c.name().to_string())
                            .collect();
                        let cols_json = serde_json::to_string(&cols)
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        sender
                            .send(crate::database::types::QueryExecEvent::TypesResolved {
                                columns: serde_json::value::RawValue::from_string(cols_json)
                                    .unwrap(),
                            })
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        columns_sent = true;
                    }
                }
                QueryItem::Row(row) => {
                    writer
                        .add_row(&row)
                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let batch = batch_size();
                    if writer.len() >= batch {
                        let page = writer.finish();
                        if env_truthy("PGPAD_VARIANT_ENRICH_BASE") {
                            pending_pages.push(page);
                        } else {
                            sender
                                .send(crate::database::types::QueryExecEvent::Page {
                                    page_amount: 0,
                                    page,
                                })
                                .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        }
                    }
                }
            }
        }

        drop(stream);
        if !writer.is_empty() {
            let page = writer.finish();
            if env_truthy("PGPAD_VARIANT_ENRICH_BASE") {
                pending_pages.push(page);
            } else {
                sender
                    .send(crate::database::types::QueryExecEvent::Page {
                        page_amount: 0,
                        page,
                    })
                    .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
            }
        }
        if env_truthy("PGPAD_VARIANT_ENRICH_BASE") {
            for mut page in pending_pages {
                page = enrich_variant_base_types(client, page).await?;
                sender
                    .send(crate::database::types::QueryExecEvent::Page {
                        page_amount: 0,
                        page,
                    })
                    .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
            }
        }
    } else {
        let result = client.execute(sql, &[]).await.map_err(|e| {
            Error::Any(anyhow::anyhow!(map_mssql_error(&format!(
                "Exec failed: {}",
                e
            ))))
        })?;
        affected_rows = result.rows_affected().iter().map(|&n| n as usize).sum();
    }

    let elapsed = now.elapsed().as_millis() as u64;
    sender
        .send(crate::database::types::QueryExecEvent::Finished {
            elapsed_ms: elapsed,
            affected_rows,
            error: None,
        })
        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;

    Ok(())
}

async fn enrich_variant_base_types(
    client: &mut Client<crate::database::mssql::connect::MssqlStream>,
    page: Box<serde_json::value::RawValue>,
) -> Result<Box<serde_json::value::RawValue>, Error> {
    let mut val: serde_json::Value =
        serde_json::from_str(page.get()).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
    // Expect page as Vec<Vec<Json>>; iterate rows and columns
    if let serde_json::Value::Array(ref mut rows) = val {
        for row in rows.iter_mut() {
            if let serde_json::Value::Array(ref mut cols) = row {
                for cell in cols.iter_mut() {
                    if let serde_json::Value::Object(ref mut obj) = cell {
                        if obj
                            .get("type")
                            .and_then(|x| x.as_str())
                            .map(|s| s == "sql_variant")
                            .unwrap_or(false)
                        {
                            let has_base = obj.get("base_type").is_some();
                            let val_str = obj.get("value").and_then(|x| x.as_str());
                            if !has_base {
                                if let Some(s) = val_str {
                                    let mut q = Query::new("SELECT SQL_VARIANT_PROPERTY(CONVERT(sql_variant, @P1), 'BaseType')");
                                    q.bind(s);
                                    let mut stream = q
                                        .query(client)
                                        .await
                                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                                    if let Some(QueryItem::Row(r)) = stream
                                        .try_next()
                                        .await
                                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
                                    {
                                        let bt: Option<&str> = r.try_get(0).map_err(|e| {
                                            Error::Any(anyhow::anyhow!(e.to_string()))
                                        })?;
                                        obj.insert(
                                            "base_type".to_string(),
                                            serde_json::Value::String(bt.unwrap_or("").to_string()),
                                        );
                                    } else {
                                        obj.insert(
                                            "base_type".to_string(),
                                            serde_json::Value::Null,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let s = serde_json::to_string(&val).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
    Ok(serde_json::value::RawValue::from_string(s).unwrap())
}

pub async fn execute_query_with_params(
    client: &mut Client<crate::database::mssql::connect::MssqlStream>,
    stmt: ParsedStatement,
    sender: &ExecSender,
    params: serde_json::Map<String, serde_json::Value>,
    settings: Option<&crate::database::types::OracleSettings>,
) -> Result<(), Error> {
    let sql = stmt.statement;
    let max_idx = find_max_param_index(&sql);
    let mut query = Query::new(&sql);

    for i in 1..=max_idx {
        let key = format!("P{}", i);
        bind_param(&mut query, params.get(&key));
    }

    let now = Instant::now();
    let mut affected_rows = 0usize;

    if stmt.explain_plan {
        client
            .simple_query("SET SHOWPLAN_XML ON")
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(format!("Enable SHOWPLAN failed: {}", e))))?;

        {
            let mut stream = query.query(client).await.map_err(|e| {
                Error::Any(anyhow::anyhow!(map_mssql_error(&format!(
                    "Prepared explain failed: {}",
                    e
                ))))
            })?;

            let mut writer = RowWriter::with_settings(settings);
            let mut columns_sent = false;
            while let Some(item) = stream
                .try_next()
                .await
                .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
            {
                match item {
                    QueryItem::Metadata(meta) => {
                        if !columns_sent {
                            let cols: Vec<String> = meta
                                .columns()
                                .iter()
                                .map(|c| c.name().to_string())
                                .collect();
                            let cols_json = serde_json::to_string(&cols)
                                .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                            sender
                                .send(crate::database::types::QueryExecEvent::TypesResolved {
                                    columns: serde_json::value::RawValue::from_string(cols_json)
                                        .unwrap(),
                                })
                                .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                            columns_sent = true;
                        }
                    }
                    QueryItem::Row(row) => {
                        writer
                            .add_row(&row)
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        let batch = batch_size();
                        if writer.len() >= batch {
                            let page = writer.finish();
                            sender
                                .send(crate::database::types::QueryExecEvent::Page {
                                    page_amount: 0,
                                    page,
                                })
                                .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        }
                    }
                }
            }

            if !writer.is_empty() {
                let page = writer.finish();
                sender
                    .send(crate::database::types::QueryExecEvent::Page {
                        page_amount: 0,
                        page,
                    })
                    .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
            }
        }
        client
            .simple_query("SET SHOWPLAN_XML OFF")
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(format!("Disable SHOWPLAN failed: {}", e))))?;
    } else if stmt.returns_values {
        let mut stream = query.query(client).await.map_err(|e| {
            Error::Any(anyhow::anyhow!(map_mssql_error(&format!(
                "Prepared query failed: {}",
                e
            ))))
        })?;

        let mut writer = RowWriter::with_settings(settings);
        let mut columns_sent = false;
        while let Some(item) = stream
            .try_next()
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
        {
            match item {
                QueryItem::Metadata(meta) => {
                    if !columns_sent {
                        let cols: Vec<String> = meta
                            .columns()
                            .iter()
                            .map(|c| c.name().to_string())
                            .collect();
                        let cols_json = serde_json::to_string(&cols)
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        sender
                            .send(crate::database::types::QueryExecEvent::TypesResolved {
                                columns: serde_json::value::RawValue::from_string(cols_json)
                                    .unwrap(),
                            })
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        columns_sent = true;
                    }
                }
                QueryItem::Row(row) => {
                    writer
                        .add_row(&row)
                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let batch = std::env::var("PGPAD_BATCH_SIZE")
                        .ok()
                        .and_then(|v| v.parse::<usize>().ok())
                        .filter(|&n| n > 0)
                        .unwrap_or(50);
                    if writer.len() >= batch {
                        let page = writer.finish();
                        sender
                            .send(crate::database::types::QueryExecEvent::Page {
                                page_amount: 0,
                                page,
                            })
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    }
                }
            }
        }

        if !writer.is_empty() {
            let page = writer.finish();
            sender
                .send(crate::database::types::QueryExecEvent::Page {
                    page_amount: 0,
                    page,
                })
                .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
        }
    } else {
        let res = query.execute(client).await.map_err(|e| {
            Error::Any(anyhow::anyhow!(map_mssql_error(&format!(
                "Prepared exec failed: {}",
                e
            ))))
        })?;
        affected_rows = res.rows_affected().iter().map(|&n| n as usize).sum();
    }

    let elapsed = now.elapsed().as_millis() as u64;
    sender
        .send(crate::database::types::QueryExecEvent::Finished {
            elapsed_ms: elapsed,
            affected_rows,
            error: None,
        })
        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;

    Ok(())
}

fn map_mssql_error(msg: &str) -> String {
    let m = msg.to_lowercase();
    let (code, hint, retry) = if m.contains("login failed") || m.contains("18456") {
        (
            "18456",
            "Authentication failed. Check username/password",
            false,
        )
    } else if m.contains("timeout") {
        (
            "ETIMEOUT",
            "Query timed out. Consider optimizing or increasing timeout",
            true,
        )
    } else if m.contains("could not find stored procedure") || m.contains("2812") {
        ("2812", "Stored procedure not found", false)
    } else if m.contains("invalid object name") || m.contains("208") {
        (
            "208",
            "Invalid object name (table/view doesn’t exist)",
            false,
        )
    } else if m.contains("permission") || m.contains("denied") {
        (
            "EPERM",
            "Insufficient permissions for requested operation",
            false,
        )
    } else {
        ("EUNKNOWN", "Unhandled MSSQL error", true)
    };
    let payload = serde_json::json!({
        "code": code,
        "message": msg,
        "hint": hint,
        "retry": retry,
    });
    payload.to_string()
}

fn find_max_param_index(sql: &str) -> usize {
    let bytes = sql.as_bytes();
    let mut i = 0usize;
    let mut max_idx = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'@'
            && i + 2 < bytes.len()
            && bytes[i + 1] == b'P'
            && bytes[i + 2].is_ascii_digit()
        {
            let mut j = i + 2;
            let mut num = 0usize;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                num = num * 10 + (bytes[j] - b'0') as usize;
                j += 1;
            }
            if num > max_idx {
                max_idx = num;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    max_idx
}

fn bind_param(query: &mut Query<'_>, v: Option<&serde_json::Value>) {
    match v {
        None => query.bind(Option::<i32>::None),
        Some(serde_json::Value::Null) => query.bind(Option::<i32>::None),
        Some(serde_json::Value::Bool(b)) => query.bind(*b),
        Some(serde_json::Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                query.bind(i);
            } else if let Some(f) = n.as_f64() {
                query.bind(f);
            } else {
                query.bind(n.to_string());
            }
        }
        Some(serde_json::Value::String(s)) => {
            if let Ok(u) = Uuid::parse_str(s) {
                query.bind(u);
            } else if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                query.bind(dt.with_timezone(&Utc));
            } else if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
                query.bind(ndt);
            } else if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
                query.bind(ndt);
            } else if let Ok(nd) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                query.bind(nd);
            } else {
                query.bind(s.clone());
            }
        }
        Some(other) => query.bind(other.to_string()),
    }
}
