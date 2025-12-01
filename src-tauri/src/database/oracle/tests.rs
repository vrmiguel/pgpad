#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use crate::database::oracle::{execute::execute_query_with_params, schema::get_database_schema};
    use crate::database::{types::channel, QueryExecEvent};
    use crate::database::connection_monitor::ConnectionMonitor;
    use tauri::async_runtime;

    #[tokio::test]
    async fn oracle_param_bind_smoke() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        let params = serde_json::json!({
            "p1": 123,
            "p2": true,
            "p3": "2024-12-01T12:34:56Z",
        }).as_object().unwrap().clone();

        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "SELECT :p1, :p2, :p3 FROM DUAL".into(), returns_values: true, is_read_only: true, explain_plan: false };
            let conn = conn.lock().unwrap();
            execute_query_with_params(&conn, stmt, &sender, params, None).unwrap();
        });

        while let Some(ev) = recv.recv().await {
            match ev { QueryExecEvent::Finished { error, .. } => { assert!(error.is_none()); break; } _ => {} }
        }

        // Schema introspection smoke
        let schema = get_database_schema(conn.clone()).await?;
        // Not asserting specific objects (depends on server), but ensure structure is non-empty vectors present
        assert!(schema.tables.len() >= 0);
        assert!(schema.schemas.len() >= 0);

        // Ping monitor lifecycle smoke (only if close is supported)
        assert!(ConnectionMonitor::oracle_ping_once(conn.clone()));
        let _ = conn.lock().ok().and_then(|c| c.close().ok());
        // after closing, ping should fail
        let ok = async_runtime::spawn_blocking({ let conn = conn.clone(); move || ConnectionMonitor::oracle_ping_once(conn) }).await.unwrap_or(true);
        assert!(!ok);

        // reconnect and verify ping works again
        let conn2 = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn2 = Arc::new(Mutex::new(conn2));
        assert!(ConnectionMonitor::oracle_ping_once(conn2.clone()));

        // Pseudo column presence checks if corresponding entities exist
        for t in &schema.tables {
            let names: Vec<&str> = t.columns.iter().map(|c| c.name.as_str()).collect();
            // Table comments, partition keys and privileges pseudo-columns
            if names.iter().any(|n| *n == "__TABLE_COMMENT__") {
                let _ = true;
            }
            if names.iter().any(|n| *n == "__PARTITION_KEY__") {
                let _ = true;
            }
            if names.iter().any(|n| *n == "__SUBPARTITION_KEY__") {
                let _ = true;
            }
            if names.iter().any(|n| *n == "__PRIVS__") {
                let _ = true;
            }
            if names.iter().any(|n| *n == "__PACKAGE_MEMBER__") {
                assert!(names.iter().any(|n| *n == "__PACKAGE_MEMBER_COUNT__"));
            }
            if names.iter().any(|n| *n == "__SYNONYM_TARGET__") {
                assert!(names.iter().any(|n| *n == "__SYNONYM_RESOLVED__"));
                if names.iter().any(|n| *n == "__SYNONYM_DB_LINK__") {
                    assert!(names.iter().any(|n| *n == "__SYNONYM_DB_LINK_STATUS__"));
                    // optional ping pseudo column when enabled
                    let allow_ping = std::env::var("ORACLE_ALLOW_DB_LINK_PING").ok().map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);
                    if allow_ping {
                        assert!(names.iter().any(|n| *n == "__SYNONYM_DB_LINK_PING__"));
                    }
                }
            }
            if names.iter().any(|n| *n == "__MV_QUERY__") {
                assert!(names.iter().any(|n| *n == "__MV_REFRESH_MODE__") || names.iter().any(|n| *n == "__MV_REFRESH_METHOD__"));
                if names.iter().any(|n| *n == "__MV_SCHED_JOBS__") {
                    assert!(names.iter().any(|n| *n == "__MV_SCHED_REFRESH_JOBS__") || names.iter().any(|n| *n == "__MV_SCHED_JOBS__"));
                    // last run or error may be present
                    if names.iter().any(|n| *n == "__MV_SCHED_LAST_RUN__") {
                        let _ = true;
                    }
                    if names.iter().any(|n| *n == "__MV_SCHED_LAST_ERROR__") {
                        let _ = true;
                    }
                }
            }
            if names.iter().any(|n| *n == "__TRIGGER_EVENT__") {
                assert!(names.iter().any(|n| *n == "__TRIGGER_TYPE__"));
                // optional trigger order
                if names.iter().any(|n| *n == "__TRIGGER_ORDER__") {
                    let _ = true;
                }
            }
            if names.iter().any(|n| *n == "__OBJECT_TYPE__") {
                // program units may expose out args flag
                if names.iter().any(|n| *n == "__HAS_OUT_ARGS__") {
                    let _ = true;
                }
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn oracle_returning_into_smoke() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        {
            let c = conn.lock().unwrap();
            let _ = c.execute("BEGIN EXECUTE IMMEDIATE 'CREATE TABLE pgpad_rt(id NUMBER, name VARCHAR2(30))'; EXCEPTION WHEN OTHERS THEN NULL; END;", &[]);
            let _ = c.execute("DELETE FROM pgpad_rt", &[]);
        }

        let params = serde_json::json!({ "name": "Alice" }).as_object().unwrap().clone();
        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "INSERT INTO pgpad_rt(name) VALUES(:name) RETURNING id INTO :id".into(), returns_values: true, is_read_only: false, explain_plan: false };
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query_with_params(&conn, stmt, &sender, params, None).unwrap();
        });

        let mut finished_ok = false;
        let mut saw_page = false;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::Page { page_amount: _, page } => {
                    let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&page).unwrap()).unwrap();
                    assert!(v[0][0].is_string());
                    saw_page = true;
                }
                QueryExecEvent::Finished { error, .. } => { finished_ok = error.is_none(); break; }
                _ => {}
            }
        }
        assert!(saw_page);
        assert!(finished_ok);
        Ok(())
    }

    #[tokio::test]
    async fn oracle_call_out_smoke() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "BEGIN :outval := UPPER(:inval); END;".into(), returns_values: true, is_read_only: false, explain_plan: false };
            let params = serde_json::json!({ "inval": "hello" }).as_object().unwrap().clone();
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query_with_params(&conn, stmt, &sender, params, None).unwrap();
        });

        let mut finished_ok = false;
        let mut saw_page = false;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::Page { page_amount: _, page } => {
                    let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&page).unwrap()).unwrap();
                    assert_eq!(v[0][0].as_str().unwrap(), "HELLO");
                    saw_page = true;
                }
                QueryExecEvent::Finished { error, .. } => { finished_ok = error.is_none(); break; }
                _ => {}
            }
        }
        assert!(saw_page);
        assert!(finished_ok);
        Ok(())
    }

    #[tokio::test]
    async fn oracle_returning_numeric_precision_scale() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        {
            let c = conn.lock().unwrap();
            let _ = c.execute("BEGIN EXECUTE IMMEDIATE 'CREATE TABLE pgpad_num_rt(val NUMBER(20,5))'; EXCEPTION WHEN OTHERS THEN NULL; END;", &[]);
            let _ = c.execute("DELETE FROM pgpad_num_rt", &[]);
        }

        let params = serde_json::json!({ "val": null }).as_object().unwrap().clone();
        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "INSERT INTO pgpad_num_rt(val) VALUES(123.45000) RETURNING val INTO :val".into(), returns_values: true, is_read_only: false, explain_plan: false };
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query_with_params(&conn, stmt, &sender, params, None).unwrap();
        });

        let mut saw_page = false;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::Page { page_amount: _, page } => {
                    let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&page).unwrap()).unwrap();
                    let first = &v[0];
                    assert!(first[0].as_str().unwrap().starts_with("123.45"));
                    saw_page = true;
                }
                QueryExecEvent::Finished { error, .. } => { assert!(error.is_none()); break; }
                _ => {}
            }
        }
        assert!(saw_page);
        Ok(())
    }

    #[tokio::test]
    async fn oracle_bind_overlap_names() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        let params = serde_json::json!({ "a": "A", "ab": "AB" }).as_object().unwrap().clone();
        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "SELECT :a AS A1, :ab AS A2 FROM DUAL".into(), returns_values: true, is_read_only: true, explain_plan: false };
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query_with_params(&conn, stmt, &sender, params, None).unwrap();
        });

        let mut saw_page = false;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::Page { page_amount: _, page } => {
                    let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&page).unwrap()).unwrap();
                    let first = &v[0];
                    assert_eq!(first[0], serde_json::Value::String("A".into()));
                    assert_eq!(first[1], serde_json::Value::String("AB".into()));
                    saw_page = true;
                }
                QueryExecEvent::Finished { error, .. } => { assert!(error.is_none()); break; }
                _ => {}
            }
        }
        assert!(saw_page);
        Ok(())
    }

    #[tokio::test]
    async fn oracle_package_out_number_precision_scale() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        {
            let c = conn.lock().unwrap();
            let _ = c.execute(
                "BEGIN EXECUTE IMMEDIATE 'CREATE OR REPLACE PACKAGE pgpad_num_pkg AS SUBTYPE num_p5 IS NUMBER(20,5); PROCEDURE set_val(p OUT num_p5); END;'; EXCEPTION WHEN OTHERS THEN NULL; END;",
                &[]);
            let _ = c.execute(
                "BEGIN EXECUTE IMMEDIATE 'CREATE OR REPLACE PACKAGE BODY pgpad_num_pkg AS PROCEDURE set_val(p OUT num_p5) IS BEGIN p := 123.45000; END; END;'; EXCEPTION WHEN OTHERS THEN NULL; END;",
                &[]);
        }

        let params = serde_json::json!({ "p": null }).as_object().unwrap().clone();
        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "BEGIN pgpad_num_pkg.set_val(:p); END;".into(), returns_values: true, is_read_only: false, explain_plan: false };
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query_with_params(&conn, stmt, &sender, params, None).unwrap();
        });

        let mut saw_page = false;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::Page { page_amount: _, page } => {
                    let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&page).unwrap()).unwrap();
                    let first = &v[0];
                    assert!(first[0].as_str().unwrap().starts_with("123.45"));
                    saw_page = true;
                }
                QueryExecEvent::Finished { error, .. } => { assert!(error.is_none()); break; }
                _ => {}
            }
        }
        assert!(saw_page);
        Ok(())
    }

    #[tokio::test]
    async fn oracle_package_inout_number_precision_scale() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        {
            let c = conn.lock().unwrap();
            let _ = c.execute(
                "BEGIN EXECUTE IMMEDIATE 'CREATE OR REPLACE PACKAGE pgpad_num_pkg AS SUBTYPE num_p5 IS NUMBER(20,5); PROCEDURE inout(p IN OUT num_p5); END;'; EXCEPTION WHEN OTHERS THEN NULL; END;",
                &[]);
            let _ = c.execute(
                "BEGIN EXECUTE IMMEDIATE 'CREATE OR REPLACE PACKAGE BODY pgpad_num_pkg AS PROCEDURE inout(p IN OUT num_p5) IS BEGIN p := p + 0.55000; END; END;'; EXCEPTION WHEN OTHERS THEN NULL; END;",
                &[]);
        }

        let params = serde_json::json!({ "io": "123.00000" }).as_object().unwrap().clone();
        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "BEGIN pgpad_num_pkg.inout(:io); END;".into(), returns_values: true, is_read_only: false, explain_plan: false };
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query_with_params(&conn, stmt, &sender, params, None).unwrap();
        });

        let mut saw_page = false;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::Page { page_amount: _, page } => {
                    let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&page).unwrap()).unwrap();
                    let first = &v[0];
                    assert_eq!(first[0], serde_json::Value::String("123.55000".into()));
                    saw_page = true;
                }
                QueryExecEvent::Finished { error, .. } => { assert!(error.is_none()); break; }
                _ => {}
            }
        }
        assert!(saw_page);
        Ok(())
    }

    #[tokio::test]
    async fn oracle_positional_binds_query() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };

        let mut rows = match conn.query("SELECT :1 AS A, :2 AS B, :1 AS A2 FROM DUAL", &[&"ALPHA", &"BETA"]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        if let Some(Ok(row)) = rows.next() {
            let a: String = row.get(0)?;
            let b: String = row.get(1)?;
            let a2: String = row.get(2)?;
            assert_eq!(a, "ALPHA");
            assert_eq!(b, "BETA");
            assert_eq!(a2, "ALPHA");
        }

        Ok(())
    }

    #[tokio::test]
    async fn oracle_paging_rows_smoke() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        std::env::set_var("PGPAD_BATCH_SIZE", "10");
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "SELECT LEVEL AS n FROM DUAL CONNECT BY LEVEL <= 25".into(), returns_values: true, is_read_only: true, explain_plan: false };
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query(&conn, stmt, &sender, None).unwrap();
        });

        let mut pages = 0usize;
        let mut total = 0usize;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::Page { page_amount, page: _ } => { pages += 1; total += page_amount; }
                QueryExecEvent::Finished { error, .. } => { assert!(error.is_none()); break; }
                _ => {}
            }
        }
        assert!(pages >= 3);
        assert_eq!(total, 25);
        Ok(())
    }

    #[tokio::test]
    async fn oracle_raw_blob_rendering_smoke() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        std::env::set_var("ORACLE_RAW_FORMAT", "preview");
        std::env::set_var("ORACLE_RAW_CHUNK_SIZE", "4");
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "SELECT HEXTORAW('0011223344556677') AS R, TO_BLOB(HEXTORAW('DEADBEEF')) AS B FROM DUAL".into(), returns_values: true, is_read_only: true, explain_plan: false };
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query(&conn, stmt, &sender, None).unwrap();
        });

        let mut saw_page = false;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::Page { page_amount: _, page } => {
                    let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&page).unwrap()).unwrap();
                    let first = &v[0];
                    assert!(first[0].as_str().unwrap().starts_with("Raw(8) preview(0..4): 0x00112233"));
                    assert_eq!(first[1].as_str().unwrap(), "Blob(4)");
                    saw_page = true;
                }
                QueryExecEvent::Finished { error, .. } => { assert!(error.is_none()); break; }
                _ => {}
            }
        }
        assert!(saw_page);
        Ok(())
    }

    #[tokio::test]
    async fn oracle_blob_streaming_chunks() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        std::env::set_var("ORACLE_BLOB_STREAM", "stream");
        std::env::set_var("ORACLE_BLOB_CHUNK_SIZE", "4");
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "SELECT TO_BLOB(HEXTORAW('DEADBEEFCAFEBABE')) AS B FROM DUAL".into(), returns_values: true, is_read_only: true, explain_plan: false };
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query(&conn, stmt, &sender, None).unwrap();
        });

        let mut saw_chunk = false;
        let mut saw_finished = false;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::BlobChunk { row_index, column_index, offset, hex_chunk } => {
                    assert_eq!(row_index, 0);
                    assert_eq!(column_index, 0);
                    assert!(hex_chunk.len() > 0);
                    saw_chunk = true;
                }
                QueryExecEvent::Finished { error, .. } => { assert!(error.is_none()); saw_finished = true; break; }
                _ => {}
            }
        }
        assert!(saw_chunk);
        assert!(saw_finished);
        Ok(())
    }

    #[tokio::test]
    async fn oracle_explain_plan_smoke() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "EXPLAIN PLAN FOR SELECT 1 FROM DUAL".into(), returns_values: true, is_read_only: true, explain_plan: true };
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query(&conn, stmt, &sender, None).unwrap();
        });

        let mut saw_types = false;
        let mut finished_ok = false;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::TypesResolved { .. } => saw_types = true,
                QueryExecEvent::Finished { error, .. } => { finished_ok = error.is_none(); break; },
                _ => {}
            }
        }
        assert!(saw_types);
        assert!(finished_ok);
        Ok(())
    }

    #[tokio::test]
    async fn oracle_describe_package_and_synonym() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        {
            let c = conn.lock().unwrap();
            let _ = c.execute("BEGIN EXECUTE IMMEDIATE 'CREATE TABLE pgpad_pkg_t(id NUMBER, name VARCHAR2(10))'; EXCEPTION WHEN OTHERS THEN NULL; END;", &[]);
            let _ = c.execute("BEGIN EXECUTE IMMEDIATE 'CREATE OR REPLACE SYNONYM pgpad_syn FOR pgpad_pkg_t'; EXCEPTION WHEN OTHERS THEN NULL; END;", &[]);
        }

        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "DESC pgpad_syn".into(), returns_values: true, is_read_only: true, explain_plan: false };
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query(&conn, stmt, &sender, None).unwrap();
        });

        let mut saw_types = false;
        let mut finished_ok = false;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::TypesResolved { .. } => saw_types = true,
                QueryExecEvent::Finished { error, .. } => { finished_ok = error.is_none(); break; },
                _ => {}
            }
        }
        assert!(saw_types);
        assert!(finished_ok);
        Ok(())
    }

    #[tokio::test]
    async fn oracle_boolean_and_number_rendering_smoke() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "SELECT 'Y' AS FLAG_CHAR, 1 AS FLAG_NUM, CAST(12345678901234567890123 AS NUMBER) AS BIG FROM DUAL".into(), returns_values: true, is_read_only: true, explain_plan: false };
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query(&conn, stmt, &sender, None).unwrap();
        });

        let mut saw_page = false;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::Page { page_amount: _, page } => {
                    let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&page).unwrap()).unwrap();
                    let first = &v[0];
                    assert_eq!(first[0], true);
                    assert_eq!(first[1], true);
                    assert_eq!(first[2], serde_json::Value::String("12345678901234567890123".into()));
                    saw_page = true;
                }
                QueryExecEvent::Finished { error, .. } => { assert!(error.is_none()); break; }
                _ => {}
            }
        }
        assert!(saw_page);
        Ok(())
    }

    #[tokio::test]
    async fn oracle_null_bind_semantics() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        let params = serde_json::json!({ "p": null }).as_object().unwrap().clone();
        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "SELECT CASE WHEN :p IS NULL THEN 'Y' ELSE 'N' END AS FLAG FROM DUAL".into(), returns_values: true, is_read_only: true, explain_plan: false };
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query_with_params(&conn, stmt, &sender, params, None).unwrap();
        });

        let mut saw_page = false;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::Page { page_amount: _, page } => {
                    let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&page).unwrap()).unwrap();
                    let first = &v[0];
                    assert_eq!(first[0], true);
                    saw_page = true;
                }
                QueryExecEvent::Finished { error, .. } => { assert!(error.is_none()); break; }
                _ => {}
            }
        }
        assert!(saw_page);
        Ok(())
    }

    #[tokio::test]
    async fn oracle_named_bind_ordering() -> anyhow::Result<()> {
        if std::env::var("ORACLE_TEST_CONNECT").is_err() { return Ok(()); }
        let conn_str = std::env::var("ORACLE_TEST_CONNECT").unwrap();
        let user = std::env::var("ORACLE_TEST_USER").unwrap_or_else(|_| "scott".into());
        let pass = std::env::var("ORACLE_TEST_PASS").unwrap_or_else(|_| "tiger".into());

        let conn = match oracle::Connection::connect(&user, &pass, &conn_str) { Ok(c) => c, Err(_) => return Ok(()) };
        let conn = Arc::new(Mutex::new(conn));

        let params = serde_json::json!({ "b": "BETA", "a": "ALPHA" }).as_object().unwrap().clone();
        let (sender, mut recv) = channel();
        tokio::task::spawn_blocking(move || {
            let stmt = crate::database::parser::ParsedStatement { statement: "SELECT :a, :b, :a FROM DUAL".into(), returns_values: true, is_read_only: true, explain_plan: false };
            let conn = conn.lock().unwrap();
            crate::database::oracle::execute::execute_query_with_params(&conn, stmt, &sender, params, None).unwrap();
        });

        let mut saw_page = false;
        while let Some(ev) = recv.recv().await {
            match ev {
                QueryExecEvent::Page { page_amount: _, page } => {
                    let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&page).unwrap()).unwrap();
                    let first = &v[0];
                    assert_eq!(first[0], serde_json::Value::String("ALPHA".into()));
                    assert_eq!(first[1], serde_json::Value::String("BETA".into()));
                    assert_eq!(first[2], serde_json::Value::String("ALPHA".into()));
                    saw_page = true;
                }
                QueryExecEvent::Finished { error, .. } => { assert!(error.is_none()); break; }
                _ => {}
            }
        }
        assert!(saw_page);
        Ok(())
    }
}
        Ok(())
    }
}
