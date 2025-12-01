use std::env;
use std::time::Instant;

use crate::database::{parser::ParsedStatement, types::ExecSender, QueryExecEvent};
use crate::utils::serialize_as_json_array;
use crate::Error;

use super::row_writer::RowWriter;
use oracle::sql_type::{ToSql, Timestamp};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use chrono::{Datelike, Timelike};
use serde_json::{json, value::RawValue as JsonRawValue};
// Bind helpers for Oracle temporal types
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub fn execute_query(
    client: &oracle::Connection,
    stmt: ParsedStatement,
    sender: &ExecSender,
    settings: Option<&crate::database::types::OracleSettings>,
) -> Result<(), Error> {
    let start = std::time::Instant::now();
    let upper = stmt.statement.trim().to_uppercase();
    if upper.starts_with("EXPLAIN PLAN") {
        return execute_explain_plan(client, &stmt.statement, sender, start, settings);
    }
    if upper.starts_with("DESCRIBE ") || upper.starts_with("DESC ") {
        return execute_describe(client, &stmt.statement, sender, start);
    }
    if upper.contains(" RETURNING INTO ") {
        return execute_dml_returning(client, &stmt.statement, sender, serde_json::Map::new(), start);
    }
    if upper.starts_with("CALL ") {
        return execute_call_with_outs(client, &stmt.statement, sender, serde_json::Map::new(), start);
    }
    let has_outs = !detect_out_specs(client, &stmt.statement).is_empty();
    if has_outs && upper.contains("BEGIN") {
        return execute_call_with_outs(client, &stmt.statement, sender, serde_json::Map::new(), start);
    }
    if stmt.returns_values {
        execute_query_with_results(client, &stmt.statement, sender, start, settings)
    } else {
        execute_modification_query(client, &stmt.statement, sender, start)
    }
}

pub fn execute_query_with_params(
    client: &oracle::Connection,
    stmt: ParsedStatement,
    sender: &ExecSender,
    params: serde_json::Map<String, serde_json::Value>,
    settings: Option<&crate::database::types::OracleSettings>,
) -> Result<(), Error> {
    let started_at = std::time::Instant::now();
    let upper = stmt.statement.to_uppercase();
    if upper.contains(" RETURNING INTO ") {
        return execute_dml_returning(client, &stmt.statement, sender, params, started_at);
    }
    if upper.starts_with("CALL ") {
        return execute_call_with_outs(client, &stmt.statement, sender, params, started_at);
    }
    let has_outs = !detect_out_specs(client, &stmt.statement).is_empty();
    if has_outs && upper.contains("BEGIN") {
        return execute_call_with_outs(client, &stmt.statement, sender, params, started_at);
    }
    let named: Vec<(String, Box<dyn ToSql>)> =
        params.iter().map(|(k, v)| (k.clone(), to_sql_box(v))).collect();
    let named_refs: Vec<(&str, &dyn ToSql)> =
        named.iter().map(|(k, b)| (k.as_str(), b.as_ref())).collect();

    if stmt.returns_values {
        match client.query_named(&stmt.statement, &named_refs[..]) {
            Ok(rows) => {
                let cols = rows.column_info().to_vec();
                cache_columns(&stmt.statement, cols.clone());
                let column_names = cols.iter().map(|c| c.name());
                let columns = serialize_as_json_array(column_names)?;
                sender.send(QueryExecEvent::TypesResolved { columns })?;

                let mut _total_rows = 0;
                let batch_size = settings.and_then(|s| s.batch_size).or_else(|| env::var("PGPAD_BATCH_SIZE").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&n| n>0)).unwrap_or(50);
            let mut writer = match settings { Some(s) => RowWriter::with_settings(cols.to_vec(), Some(s)).with_sender(sender.clone()), None => RowWriter::new(cols.to_vec()).with_sender(sender.clone()) };
            for (idx, row_res) in rows.enumerate() {
                let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!("Row error: {}", e)))?;
                writer.add_row(&row, idx).map_err(|e| Error::Any(anyhow::anyhow!("Row add error: {}", e)))?;
                
                let _ = idx;
                if writer.len() >= batch_size {
                    sender.send(QueryExecEvent::Page { page_amount: writer.len(), page: writer.finish() })?;
                }
            }
                if !writer.is_empty() {
                    sender.send(QueryExecEvent::Page { page_amount: writer.len(), page: writer.finish() })?;
                }
                sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: None })?;
                Ok(())
            }
            Err(e) => {
                invalidate_on_oracle_error(&stmt.statement, &e.to_string());
                let error_msg = map_oracle_error(&e.to_string());
                sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: Some(error_msg.clone()) })?;
                Err(Error::Any(anyhow::anyhow!(error_msg)))
            }
        }
    } else {
        match client.execute_named(&stmt.statement, &named_refs[..]) {
            Ok(res) => {
                let rows_affected = res.row_count().unwrap_or(0) as usize;
                sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: rows_affected, error: None })?;
                Ok(())
            }
            Err(e) => {
                invalidate_on_oracle_error(&stmt.statement, &e.to_string());
                let error_msg = map_oracle_error(&e.to_string());
                sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: Some(error_msg.clone()) })?;
                Err(Error::Any(anyhow::anyhow!(error_msg)))
            }
        }
    }
}

fn execute_dml_returning(
    client: &oracle::Connection,
    sql: &str,
    sender: &ExecSender,
    params: serde_json::Map<String, serde_json::Value>,
    started_at: Instant,
) -> Result<(), Error> {
    let mut stmt = client.statement(sql).build().map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
    let (ret_cols, out_names) = parse_returning_into(sql);
    let (owner, table_name) = parse_dml_table(sql, client);
    let col_types = if !owner.is_empty() && !table_name.is_empty() { fetch_column_types(client, &owner, &table_name, &ret_cols) } else { std::collections::HashMap::new() };
    let bind_names_owned: Vec<String> = stmt.bind_names().iter().map(|s| s.to_string()).collect();
    for name in &bind_names_owned {
        let key = name.to_lowercase();
        if let Some(v) = params.get(&key) {
            stmt.bind(name.as_str(), &*to_sql_box(v)).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        } else if let Some(idx) = out_names.iter().position(|n| n.eq_ignore_ascii_case(name)) {
            let col = &ret_cols[idx];
            if let Some(ot) = col_types.get(&col.to_uppercase()) { stmt.bind(name.as_str(), ot).map_err(|e| Error::Any(anyhow::anyhow!(e)))?; }
            else { stmt.bind(name.as_str(), &oracle::sql_type::OracleType::Varchar2(4000)).map_err(|e| Error::Any(anyhow::anyhow!(e)))?; }
        } else {
            stmt.bind(name.as_str(), &oracle::sql_type::OracleType::Varchar2(4000)).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        }
    }
    match stmt.execute(&[]) {
        Ok(()) => {
            let columns = serialize_as_json_array(ret_cols.iter().map(|s| s.as_str()))?;
            sender.send(QueryExecEvent::TypesResolved { columns })?;
            let mut values: Vec<String> = Vec::new();
            for name in &out_names {
                let v: String = stmt.bind_value(name.as_str()).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                values.push(v);
            }
            let page_json = json!([values]);
            let page = JsonRawValue::from_string(serde_json::to_string(&page_json).unwrap()).unwrap();
            sender.send(QueryExecEvent::Page { page_amount: 1, page })?;
            sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 1, error: None })?;
            Ok(())
        }
        Err(e) => {
            let error_msg = map_oracle_error(&e.to_string());
            sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: Some(error_msg.clone()) })?;
            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

fn parse_returning_into(sql: &str) -> (Vec<String>, Vec<String>) {
    let upper = sql.to_uppercase();
    let rpos = upper.find(" RETURNING ").unwrap_or(0);
    let ipos = upper.find(" INTO ").unwrap_or(upper.len());
    let mut ret_cols: Vec<String> = Vec::new();
    let mut out_names: Vec<String> = Vec::new();
    if rpos > 0 && ipos > rpos {
        let cols_part = &sql[rpos + " RETURNING ".len()..ipos];
        ret_cols = cols_part.split(',').map(|s| s.trim().to_string()).collect();
        let names_part = &sql[ipos + " INTO ".len()..];
        out_names = names_part.split(',').map(|s| s.trim().trim_start_matches(':').to_string()).collect();
    }
    (ret_cols, out_names)
}

fn parse_dml_table(sql: &str, client: &oracle::Connection) -> (String, String) {
    let up = sql.to_uppercase();
    let s = up.trim();
    let mut owner = String::new();
    let mut table = String::new();
    if let Some(rest) = s.strip_prefix("INSERT INTO ") {
        let name = rest.split_whitespace().next().unwrap_or("");
        let parts: Vec<&str> = name.trim_matches('"').split('.').collect();
        match parts.len() { 2 => { owner = parts[0].into(); table = parts[1].into(); }, 1 => { table = parts[0].into(); }, _ => {} }
    } else if let Some(rest) = s.strip_prefix("UPDATE ") {
        let name = rest.split_whitespace().next().unwrap_or("");
        let parts: Vec<&str> = name.trim_matches('"').split('.').collect();
        match parts.len() { 2 => { owner = parts[0].into(); table = parts[1].into(); }, 1 => { table = parts[0].into(); }, _ => {} }
    } else if let Some(rest) = s.strip_prefix("DELETE FROM ") {
        let name = rest.split_whitespace().next().unwrap_or("");
        let parts: Vec<&str> = name.trim_matches('"').split('.').collect();
        match parts.len() { 2 => { owner = parts[0].into(); table = parts[1].into(); }, 1 => { table = parts[0].into(); }, _ => {} }
    }
    if owner.is_empty() { if let Ok(mut r) = client.query("SELECT USER FROM DUAL", &[]) { if let Some(Ok(row)) = r.next() { owner = row.get::<usize, String>(0).unwrap_or_default().to_uppercase(); } } }
    (owner, table)
}

fn fetch_column_types(client: &oracle::Connection, owner: &str, table: &str, cols: &[String]) -> std::collections::HashMap<String, oracle::sql_type::OracleType> {
    let sql = "SELECT column_name, data_type, data_length, data_precision, data_scale FROM all_tab_columns WHERE owner = :1 AND table_name = :2";
    let mut map = std::collections::HashMap::new();
    if let Ok(mut rows) = client.query(sql, &[&owner.to_uppercase(), &table.to_uppercase()]) {
        while let Some(Ok(row)) = rows.next() {
            let name: String = row.get(0).unwrap_or(String::new()).to_uppercase();
            let dtype: String = row.get(1).unwrap_or_else(|_| String::from("VARCHAR2"));
            let dlen: i64 = row.get(2).unwrap_or(4000);
            let dprec: i64 = row.get(3).unwrap_or(0);
            let dscale: i64 = row.get(4).unwrap_or(0);
            let ot = map_oracle_type(&dtype, dlen as i32, dprec as i16, dscale as i16);
            map.insert(name, ot);
        }
    }
    let subset: std::collections::HashMap<String, oracle::sql_type::OracleType> = cols.iter().map(|c| c.to_uppercase()).filter_map(|n| map.get(&n).map(|ot| (n.clone(), ot.clone()))).collect();
    subset
}

fn execute_call_with_outs(
    client: &oracle::Connection,
    sql: &str,
    sender: &ExecSender,
    params: serde_json::Map<String, serde_json::Value>,
    started_at: Instant,
) -> Result<(), Error> {
    let mut stmt = client.statement(sql).build().map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
    let bind_names_owned: Vec<String> = stmt.bind_names().iter().map(|s| s.to_string()).collect();
    let mut out_names: Vec<String> = Vec::new();
    let specs = detect_out_specs(client, sql);
    let mut inout_holders: Vec<String> = Vec::new();
    for name in &bind_names_owned {
        let key = name.to_lowercase();
        if let Some((_, inout, ot)) = specs.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(name)) {
            if inout.eq_ignore_ascii_case("OUT") {
                out_names.push(name.to_string());
                stmt.bind(name.as_str(), ot).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            } else {
                out_names.push(name.to_string());
                if let Some(v) = params.get(&key) {
                    let init = match v {
                        serde_json::Value::Null => String::new(),
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => if *b { String::from("1") } else { String::from("0") },
                        other => other.to_string(),
                    };
                    inout_holders.push(init);
                    let s_ref = inout_holders.last_mut().unwrap();
                    stmt.bind(name.as_str(), s_ref).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                } else {
                    stmt.bind(name.as_str(), ot).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                }
            }
        } else if let Some(v) = params.get(&key) {
            stmt.bind(name.as_str(), &*to_sql_box(v)).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        } else {
            out_names.push(name.to_string());
            stmt.bind(name.as_str(), &oracle::sql_type::OracleType::Varchar2(4000)).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        }
    }
    match stmt.execute(&[]) {
        Ok(()) => {
            let columns = serialize_as_json_array(out_names.iter().map(|s| s.as_str()))?;
            sender.send(QueryExecEvent::TypesResolved { columns })?;
            let mut values: Vec<String> = Vec::new();
            for name in &out_names {
                let v: String = stmt.bind_value(name.as_str()).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                if let Some((_, _, oracle::sql_type::OracleType::Number(_, s))) = specs.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(name)) {
                    values.push(crate::database::oracle::numeric::pad_number_scale(&v, (*s).try_into().unwrap_or(0)));
                } else { values.push(v); }
            }
            let page_json = json!([values]);
            let page = JsonRawValue::from_string(serde_json::to_string(&page_json).unwrap()).unwrap();
            sender.send(QueryExecEvent::Page { page_amount: 1, page })?;
            sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: None })?;
            Ok(())
        }
        Err(e) => {
            let error_msg = map_oracle_error(&e.to_string());
            sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: Some(error_msg.clone()) })?;
            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

#[allow(dead_code)]
fn normalize_binds_cached(
    sql: &str,
    params: &serde_json::Map<String, serde_json::Value>,
) -> (String, Vec<Box<dyn ToSql>>) {
    type BindPlan = (String, Vec<String>);
    static CACHE: OnceLock<Mutex<HashMap<String, BindPlan>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Ok(map) = cache.lock() {
        if let Some((norm, names)) = map.get(sql) {
            let boxed = names.iter().map(|n| to_sql_box(params.get(n).unwrap_or(&serde_json::Value::Null))).collect();
            return (norm.clone(), boxed);
        }
    }

    let (norm, names) = normalize_binds_extract(sql);
    let boxed = names.iter().map(|n| to_sql_box(params.get(n).unwrap_or(&serde_json::Value::Null))).collect();
    if let Ok(mut map) = cache.lock() { map.insert(sql.to_string(), (norm.clone(), names)); }
    (norm, boxed)
}

#[allow(dead_code)]
fn normalize_binds(
    sql: &str,
    params: &serde_json::Map<String, serde_json::Value>,
) -> (String, Vec<Box<dyn ToSql>>) {
    let (out, names) = normalize_binds_extract(sql);
    let boxed: Vec<Box<dyn ToSql>> = names.iter().map(|n| to_sql_box(params.get(n).unwrap_or(&serde_json::Value::Null))).collect();
    (out, boxed)
}

fn normalize_binds_extract(sql: &str) -> (String, Vec<String>) {
    let mut out = String::with_capacity(sql.len());
    let mut names: Vec<String> = Vec::new();
    let mut i = 0usize;
    let bytes = sql.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '\'' && !in_double { in_single = !in_single; out.push(c); i += 1; continue; }
        if c == '"' && !in_single { in_double = !in_double; out.push(c); i += 1; continue; }
        if !in_single && !in_double && c == ':' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() {
                let ch = bytes[j] as char;
                if ch.is_ascii_alphanumeric() || ch == '_' { j += 1; } else { break; }
            }
            if j > start {
                let name = &sql[start..j];
                if name.chars().next().unwrap().is_ascii_digit() {
                    // positional, keep as-is
                    out.push(':');
                    out.push_str(name);
                } else {
                    // named -> positional
                    let pos = match names.iter().position(|n| n == name) { Some(p) => p + 1, None => { names.push(name.to_string()); names.len() } };
                    out.push(':');
                    out.push_str(&pos.to_string());
                }
                i = j;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }

    (out, names)
}

fn to_sql_box(v: &serde_json::Value) -> Box<dyn ToSql> {
    match v {
        serde_json::Value::Null => Box::new(Option::<String>::None) as Box<dyn ToSql>,
        serde_json::Value::Bool(b) => Box::new(if *b { 1i64 } else { 0i64 }) as Box<dyn ToSql>,
        serde_json::Value::Number(num) => {
            if let Some(i) = num.as_i64() { Box::new(i) as Box<dyn ToSql> } else { Box::new(num.as_f64().unwrap_or(0.0)) as Box<dyn ToSql> }
        }
        serde_json::Value::String(s) => {
            if let Some(ts) = parse_timestamp(s) { Box::new(ts) as Box<dyn ToSql> }
            else { Box::new(s.clone()) as Box<dyn ToSql> }
        }
        other => Box::new(other.to_string()) as Box<dyn ToSql>,
    }
}

fn parse_timestamp(s: &str) -> Option<Timestamp> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        let dt_utc: DateTime<Utc> = dt.with_timezone(&Utc);
        return Some(Timestamp::new(
            dt_utc.year(),
            dt_utc.month(),
            dt_utc.day(),
            dt_utc.hour(),
            dt_utc.minute(),
            dt_utc.second(),
            dt_utc.timestamp_subsec_nanos(),
        ).expect("valid RFC3339 timestamp"));
    }

    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(Timestamp::new(
            ndt.date().year(),
            ndt.date().month(),
            ndt.date().day(),
            ndt.time().hour(),
            ndt.time().minute(),
            ndt.time().second(),
            ndt.time().nanosecond(),
        ).expect("valid naive datetime"));
    }

    if let Ok(_nd) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return None;
    }

    None
}

// TimestampTZ binding not supported; fallback handled via parse_timestamp

// Date binding not supported; treat as string

fn execute_query_with_results(
    client: &oracle::Connection,
    query: &str,
    sender: &ExecSender,
    started_at: Instant,
    settings: Option<&crate::database::types::OracleSettings>,
) -> Result<(), Error> {
    match client.query(query, &[]) {
        Ok(rows) => {
            let cols = rows.column_info();
            cache_columns(query, cols.to_vec());
            let column_names = cols.iter().map(|c| c.name());
            let columns = serialize_as_json_array(column_names)?;
            sender.send(QueryExecEvent::TypesResolved { columns })?;

            let mut total_rows = 0;
            let batch_size = settings
                .and_then(|s| s.batch_size)
                .or_else(|| env::var("PGPAD_BATCH_SIZE").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&n| n > 0))
                .unwrap_or(50);
            let mut writer = match settings { Some(s) => RowWriter::with_settings(cols.to_vec(), Some(s)).with_sender(sender.clone()), None => RowWriter::new(cols.to_vec()).with_sender(sender.clone()) };

            for (idx, row_res) in rows.enumerate() {
                let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!("Row error: {}", e)))?;
                writer.add_row(&row, idx).map_err(|e| Error::Any(anyhow::anyhow!("Row add error: {}", e)))?;
                total_rows += 1;
                if writer.len() >= batch_size {
                    sender.send(QueryExecEvent::Page { page_amount: writer.len(), page: writer.finish() })?;
                }
            }

            if !writer.is_empty() {
                sender.send(QueryExecEvent::Page { page_amount: writer.len(), page: writer.finish() })?;
            }

            let duration = started_at.elapsed().as_millis() as u64;
            log::info!("Oracle query completed: {} rows in {}ms", total_rows, duration);
            sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: None })?;
            Ok(())
        }
        Err(e) => {
            invalidate_on_oracle_error(query, &e.to_string());
            let error_msg = map_oracle_error(&e.to_string());
            sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: Some(error_msg.clone()) })?;
            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

fn execute_modification_query(
    client: &oracle::Connection,
    query: &str,
    sender: &ExecSender,
    started_at: Instant,
) -> Result<(), Error> {
    match client.execute(query, &[]) {
        Ok(res) => {
            let rows_affected = res.row_count().unwrap_or(0) as usize;
            sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: rows_affected, error: None })?;
            Ok(())
        }
        Err(e) => {
            invalidate_on_oracle_error(query, &e.to_string());
            let error_msg = map_oracle_error(&e.to_string());
            sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: Some(error_msg.clone()) })?;
            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

fn execute_explain_plan(
    client: &oracle::Connection,
    stmt: &str,
    sender: &ExecSender,
    started_at: Instant,
    settings: Option<&crate::database::types::OracleSettings>,
) -> Result<(), Error> {
    match client.execute(stmt, &[]) {
        Ok(_) => {
            let fmt = settings.and_then(|s| s.xplan_format.clone()).unwrap_or_else(|| std::env::var("ORACLE_XPLAN_FORMAT").unwrap_or_else(|_| "TYPICAL".to_string()));
            let mode = settings.and_then(|s| s.xplan_mode.clone()).unwrap_or_else(|| String::from("display"));
            let q = if mode.eq_ignore_ascii_case("display_cursor") {
                format!("SELECT * FROM TABLE(DBMS_XPLAN.DISPLAY_CURSOR(NULL, NULL, '{}'))", fmt)
            } else {
                format!("SELECT * FROM TABLE(DBMS_XPLAN.DISPLAY(NULL, NULL, '{}'))", fmt)
            };
            match client.query(&q, &[]) {
                Ok(rows) => {
                    // Collect DBMS_XPLAN lines into a vector of strings
                    let mut lines: Vec<String> = Vec::new();
                    for r in rows {
                        let row = r.map_err(|e| Error::Any(anyhow::anyhow!("Row error: {}", e)))?;
                        // Typically single column PLAN_TABLE_OUTPUT
                        let s: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                        lines.push(s);
                    }

                    // Parse lines into structured rows
                    let (cols, page_json) = format_dbms_xplan(&lines);
                    let columns = serialize_as_json_array(cols.iter().map(|s| s.as_str()))?;
                    sender.send(QueryExecEvent::TypesResolved { columns })?;
                    let page = JsonRawValue::from_string(serde_json::to_string(&page_json).unwrap()).unwrap();
                    sender.send(QueryExecEvent::Page { page_amount: page_json.as_array().map(|a| a.len()).unwrap_or(0), page })?;
                    sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: None })?;
                    Ok(())
                }
                Err(e) => {
                    let error_msg = format!("Explain plan fetch failed: {}", e);
                    sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: Some(error_msg.clone()) })?;
                    Err(Error::Any(anyhow::anyhow!(error_msg)))
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Explain plan failed: {}", e);
            sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: Some(error_msg.clone()) })?;
            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

fn format_dbms_xplan(lines: &[String]) -> (Vec<String>, serde_json::Value) {
    // Try to detect tabular section separated by '|' columns
    let mut header_idx = None;
    for (i, line) in lines.iter().enumerate() {
        if line.contains("Operation") && line.contains("Id") && line.contains("Cost") && line.contains("|") {
            header_idx = Some(i);
            break;
        }
    }
    if let Some(hidx) = header_idx {
        // Column names from header line
        let headers: Vec<String> = lines[hidx]
            .split('|')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let mut rows_json: Vec<serde_json::Value> = Vec::new();
        for line in lines.iter().skip(hidx + 1) {
            if !line.starts_with('|') { break; }
            let fields: Vec<String> = line
                .split('|')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if fields.len() >= headers.len() {
                let mut obj = serde_json::Map::new();
                for (h, v) in headers.iter().zip(fields.iter()) {
                    obj.insert(h.clone(), serde_json::Value::String(v.clone()));
                }
                rows_json.push(serde_json::Value::Object(obj));
            }
        }
        return (headers, serde_json::Value::Array(rows_json));
    }
    // Fallback: return raw lines as a single column
    let headers = vec!["Plan".to_string()];
    let arr = serde_json::Value::Array(lines.iter().map(|s| serde_json::Value::Array(vec![serde_json::Value::String(s.clone())])).collect());
    (headers, arr)
}

fn execute_describe(
    client: &oracle::Connection,
    stmt: &str,
    sender: &ExecSender,
    started_at: Instant,
) -> Result<(), Error> {
    let s = stmt.trim();
    let name = if s.to_uppercase().starts_with("DESCRIBE ") { &s[9..] } else { &s[5..] };
    let obj = name.trim();
    let (owner, obj_name) = match obj.split_once('.') {
        Some((o, n)) => (o.trim().to_uppercase(), n.trim().to_uppercase()),
        None => {
            let owner = match client.query("SELECT USER FROM DUAL", &[]) {
                Ok(mut r) => match r.next() { Some(Ok(row)) => row.get::<usize, String>(0).unwrap_or_default().to_uppercase(), _ => String::new() },
                Err(_) => String::new(),
            };
            (owner, obj.to_uppercase())
        }
    };
    // Check object type
    let obj_type = match client.query("SELECT object_type FROM all_objects WHERE owner = :1 AND object_name = :2", &[&owner, &obj_name]) {
        Ok(mut r) => match r.next() { Some(Ok(row)) => row.get::<usize, String>(0).unwrap_or_else(|_| "".into()).to_uppercase(), _ => String::new() },
        Err(_) => String::new(),
    };
    // Resolve synonyms to target object
    let (owner, obj_name) = if obj_type == "SYNONYM" {
        let sql_syn = "SELECT table_owner, table_name, db_link FROM all_synonyms WHERE owner = :1 AND synonym_name = :2";
        match client.query(sql_syn, &[&owner, &obj_name]) {
            Ok(mut r) => match r.next() {
                Some(Ok(row)) => {
                    let t_owner: String = row.get(0).unwrap_or_else(|_| owner.clone());
                    let t_name: String = row.get(1).unwrap_or_else(|_| obj_name.clone());
                    (t_owner.to_uppercase(), t_name.to_uppercase())
                }
                _ => (owner, obj_name),
            },
            Err(_) => (owner, obj_name),
        }
    } else { (owner, obj_name) };
    // If package, list subprogram arguments
    if obj_type == "PACKAGE" || obj_type == "PACKAGE BODY" {
        let sql = "SELECT a.object_name AS subprogram, NVL(a.argument_name, '<return>') AS argument_name, a.position, a.in_out, a.data_type \
                   FROM all_arguments a \
                   WHERE a.owner = :1 AND a.package_name = :2 \
                   ORDER BY a.object_name, a.position";
        match client.query(sql, &[&owner, &obj_name]) {
            Ok(rows) => {
                let cols = rows.column_info();
                cache_columns(sql, cols.to_vec());
                let column_names = cols.iter().map(|c| c.name());
                let columns = serialize_as_json_array(column_names)?;
                sender.send(QueryExecEvent::TypesResolved { columns })?;

                let batch_size = env::var("PGPAD_BATCH_SIZE").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&n| n > 0).unwrap_or(50);
                let mut writer = RowWriter::new(cols.to_vec()).with_sender(sender.clone());
                for (idx, row_res) in rows.enumerate() {
                    let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!("Row error: {}", e)))?;
                    writer.add_row(&row, idx).map_err(|e| Error::Any(anyhow::anyhow!("Row add error: {}", e)))?;
                    if writer.len() >= batch_size {
                        sender.send(QueryExecEvent::Page { page_amount: writer.len(), page: writer.finish() })?;
                    }
                }
                if !writer.is_empty() {
                    sender.send(QueryExecEvent::Page { page_amount: writer.len(), page: writer.finish() })?;
                }
                sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: None })?;
                return Ok(());
            }
            Err(e) => {
                let error_msg = format!("Describe package failed: {}", e);
                sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: Some(error_msg.clone()) })?;
                return Err(Error::Any(anyhow::anyhow!(error_msg)));
            }
        }
    }

    let sql = "SELECT c.column_name, c.data_type, c.nullable, c.data_default, com.comments \
               FROM all_tab_columns c \
               LEFT JOIN all_col_comments com \
                 ON com.owner = c.owner AND com.table_name = c.table_name AND com.column_name = c.column_name \
               WHERE c.owner = :1 AND c.table_name = :2 ORDER BY c.column_id";
    match client.query(sql, &[&owner, &obj_name]) {
        Ok(rows) => {
            let cols = rows.column_info();
            cache_columns(sql, cols.to_vec());
            let column_names = cols.iter().map(|c| c.name());
            let columns = serialize_as_json_array(column_names)?;
            sender.send(QueryExecEvent::TypesResolved { columns })?;
            let batch_size = env::var("PGPAD_BATCH_SIZE").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&n| n>0).unwrap_or(50);
            let mut writer = RowWriter::new(cols.to_vec()).with_sender(sender.clone());
            for (idx, row_res) in rows.enumerate() {
                let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!("Row error: {}", e)))?;
                writer.add_row(&row, idx).map_err(|e| Error::Any(anyhow::anyhow!("Row add error: {}", e)))?;
                if writer.len() >= batch_size {
                    sender.send(QueryExecEvent::Page { page_amount: writer.len(), page: writer.finish() })?;
                }
            }
            if !writer.is_empty() {
                sender.send(QueryExecEvent::Page { page_amount: writer.len(), page: writer.finish() })?;
            }
            sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: None })?;
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Describe failed: {}", e);
            sender.send(QueryExecEvent::Finished { elapsed_ms: started_at.elapsed().as_millis() as u64, affected_rows: 0, error: Some(error_msg.clone()) })?;
            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

// Columns metadata cache keyed by normalized SQL
type ColumnsCacheEntry = Vec<oracle::ColumnInfo>;
static COLUMNS_CACHE: OnceLock<Mutex<HashMap<String, ColumnsCacheEntry>>> = OnceLock::new();

fn cache_columns(sql: &str, cols: Vec<oracle::ColumnInfo>) {
    let cache = COLUMNS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut m) = cache.lock() {
        if m.len() > 256 { m.clear(); }
        m.insert(sql.to_string(), cols);
    }
}

fn invalidate_on_oracle_error(sql: &str, err_msg: &str) {
    let should_invalidate = err_msg.contains("ORA-04061")
        || err_msg.contains("ORA-04065")
        || err_msg.contains("ORA-01036")
        || err_msg.contains("ORA-00904");
    if should_invalidate {
        if let Some(cache) = COLUMNS_CACHE.get() {
            if let Ok(mut m) = cache.lock() {
                m.remove(sql);
            }
        }
    }
}

pub fn map_oracle_error(e: &str) -> String {
    let msg = e.to_string();
    let hint = if msg.contains("ORA-01017") {
        "Invalid username/password. Check credentials or keyring entry."
    } else if msg.contains("ORA-12154") {
        "TNS alias not resolved. Verify tnsnames.ora and TNS_ADMIN or tns_alias setting."
    } else if msg.contains("ORA-12514") {
        "Listener does not know requested service. Confirm service name/SID in connect string."
    } else if msg.contains("ORA-12541") {
        "No listener. Ensure the listener is running and reachable."
    } else if msg.contains("ORA-28040") {
        "Auth protocol mismatch. Update client/driver or server auth settings."
    } else if msg.contains("ORA-29024") {
        "Wallet/certificate validation failure. Verify wallet path and trust store."
    } else if msg.contains("ORA-06550") {
        "PL/SQL compilation error. Inspect accompanying errors for line details."
    } else if msg.contains("ORA-02019") {
        "DB link resolution error. Check remote link privileges and availability."
    } else {
        ""
    };
    if hint.is_empty() { format!("Query failed: {}", msg) } else { format!("{} Hint: {}", msg, hint) }
}
pub fn detect_out_args(client: &oracle::Connection, sql: &str) -> Vec<String> {
    // Try to identify owner/package/object from CALL or block
    let up = sql.to_uppercase();
    let target = if up.starts_with("CALL ") { sql.trim()[5..].trim().to_string() } else {
        // naive extraction inside BEGIN...END; use first token containing '('
        let s = sql.trim();
        if let Some(beg) = s.find('(') {
            let before = &s[..beg];
            // last word segment
            let parts: Vec<&str> = before.split_whitespace().collect();
            parts.last().map(|x| x.to_string()).unwrap_or_default()
        } else { String::new() }
    };
    let name = target.trim().trim_end_matches(';');
    let mut owner = String::new();
    let mut package = String::new();
    let mut object = String::new();
    if !name.is_empty() {
        let parts: Vec<&str> = name.split('.').collect();
        match parts.len() {
            3 => { owner = parts[0].trim().to_uppercase(); package = parts[1].trim().to_uppercase(); object = parts[2].trim().to_uppercase(); }
            2 => { package = parts[0].trim().to_uppercase(); object = parts[1].trim().to_uppercase(); },
            1 => { object = parts[0].trim().to_uppercase(); },
            _ => {}
        }
        if owner.is_empty() {
            if let Ok(mut r) = client.query("SELECT USER FROM DUAL", &[]) { if let Some(Ok(row)) = r.next() { owner = row.get::<usize, String>(0).unwrap_or_default().to_uppercase(); } }
        }
        let sql = if package.is_empty() {
            "SELECT NVL(argument_name, '<anonymous>') FROM all_arguments WHERE owner = :1 AND object_name = :2 AND UPPER(in_out) IN ('OUT','IN/OUT') ORDER BY position"
        } else {
            "SELECT NVL(argument_name, '<anonymous>') FROM all_arguments WHERE owner = :1 AND object_name = :2 AND package_name = :3 AND UPPER(in_out) IN ('OUT','IN/OUT') ORDER BY position"
        };
        let binds: Vec<&dyn ToSql> = if package.is_empty() { vec![&owner, &object] } else { vec![&owner, &object, &package] };
        if let Ok(mut rows) = client.query(sql, &binds[..]) {
            let mut names = Vec::new();
            while let Some(Ok(row)) = rows.next() {
                if let Ok(n) = row.get::<usize, String>(0) { names.push(n); }
            }
            return names;
        }
    }
    Vec::new()
}

type OutSpecs = Vec<(String, String, oracle::sql_type::OracleType)>;
pub fn detect_out_specs(client: &oracle::Connection, sql: &str) -> OutSpecs {
    static OUT_SPECS_CACHE: OnceLock<Mutex<HashMap<String, OutSpecs>>> = OnceLock::new();
    let cache = OUT_SPECS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(m) = cache.lock() { if let Some(v) = m.get(sql) { return v.clone(); } }
    let up = sql.to_uppercase();
    let target = if up.starts_with("CALL ") { sql.trim()[5..].trim().to_string() } else {
        let s = sql.trim();
        if let Some(beg) = s.find('(') { let before = &s[..beg]; let parts: Vec<&str> = before.split_whitespace().collect(); parts.last().map(|x| x.to_string()).unwrap_or_default() } else { String::new() }
    };
    let name = target.trim().trim_end_matches(';');
    let mut owner = String::new();
    let mut package = String::new();
    let mut object = String::new();
    if !name.is_empty() {
        let parts: Vec<&str> = name.split('.').collect();
        match parts.len() { 3 => { owner = parts[0].trim().to_uppercase(); package = parts[1].trim().to_uppercase(); object = parts[2].trim().to_uppercase(); }, 2 => { package = parts[0].trim().to_uppercase(); object = parts[1].trim().to_uppercase(); }, 1 => { object = parts[0].trim().to_uppercase(); }, _ => {} }
        if owner.is_empty() { if let Ok(mut r) = client.query("SELECT USER FROM DUAL", &[]) { if let Some(Ok(row)) = r.next() { owner = row.get::<usize, String>(0).unwrap_or_default().to_uppercase(); } } }
        let sql = if package.is_empty() {
            "SELECT NVL(argument_name, '<anonymous>'), in_out, data_type, data_length, data_precision, data_scale FROM all_arguments WHERE owner = :1 AND object_name = :2 AND UPPER(in_out) IN ('OUT','IN/OUT') ORDER BY position"
        } else {
            "SELECT NVL(argument_name, '<anonymous>'), in_out, data_type, data_length, data_precision, data_scale FROM all_arguments WHERE owner = :1 AND object_name = :2 AND package_name = :3 AND UPPER(in_out) IN ('OUT','IN/OUT') ORDER BY position"
        };
        let binds: Vec<&dyn ToSql> = if package.is_empty() { vec![&owner, &object] } else { vec![&owner, &object, &package] };
        if let Ok(mut rows) = client.query(sql, &binds[..]) {
            let mut specs = Vec::new();
            while let Some(Ok(row)) = rows.next() {
                let name: String = row.get(0).unwrap_or(String::new());
                let inout: String = row.get(1).unwrap_or(String::new());
                let dtype: String = row.get(2).unwrap_or_else(|_| String::from("VARCHAR2"));
                let dlen: i64 = row.get(3).unwrap_or(4000);
                let dprec: i64 = row.get(4).unwrap_or(0);
                let dscale: i64 = row.get(5).unwrap_or(0);
                let ot = map_oracle_type(&dtype, dlen as i32, dprec as i16, dscale as i16);
                specs.push((name, inout, ot));
            }
            if let Ok(mut m) = cache.lock() { m.insert(sql.to_string(), specs.clone()); }
            return specs;
        }
    }
    Vec::new()
}

fn map_oracle_type(dtype: &str, len: i32, prec: i16, scale: i16) -> oracle::sql_type::OracleType {
    let t = dtype.to_uppercase();
    if t.contains("VARCHAR") || t == "CHAR" { oracle::sql_type::OracleType::Varchar2(std::cmp::max(1u32, len.try_into().unwrap_or(1))) }
    else if t == "NUMBER" { let p = if prec <= 0 { 38 } else { prec }; oracle::sql_type::OracleType::Number(p.try_into().unwrap(), scale.try_into().unwrap()) }
    else if t == "DATE" { oracle::sql_type::OracleType::Varchar2(64) }
    else if t.contains("TIMESTAMP") { oracle::sql_type::OracleType::Varchar2(128) }
    else if t == "RAW" { oracle::sql_type::OracleType::Raw(std::cmp::max(1u32, len.try_into().unwrap_or(1))) }
    else if t == "BLOB" { oracle::sql_type::OracleType::BLOB }
    else if t == "CLOB" { oracle::sql_type::OracleType::CLOB }
    else { oracle::sql_type::OracleType::Varchar2(std::cmp::max(1u32, len.try_into().unwrap_or(1))) }
}
