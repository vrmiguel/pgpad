#![cfg(feature = "mssql_tests")]

use crate::database::{mssql, Certificates};

#[tokio::test]
async fn test_mssql_connect_and_select() {
    let url = std::env::var("MSSQL_TEST_URL").ok();
    if url.is_none() { return; }
    let certs = Certificates::new();
    let client = mssql::connect::connect(&url.unwrap(), &certs, None, None).await.unwrap();
    let mut stream = client.simple_query("SELECT 1 AS a").await.unwrap();
    let row = stream.into_row().await.unwrap().unwrap();
    let v: Option<i32> = row.try_get(0).unwrap();
    assert_eq!(v, Some(1));
}

#[tokio::test]
async fn test_mssql_prepared_params() {
    let url = std::env::var("MSSQL_TEST_URL").ok();
    if url.is_none() { return; }
    let certs = Certificates::new();
    let mut client = mssql::connect::connect(&url.unwrap(), &certs, None, None).await.unwrap();
    let mut q = tiberius::Query::new("SELECT @P1 AS x, @P2 AS y");
    q.bind(7i32);
    q.bind("ok");
    let mut stream = q.query(&mut client).await.unwrap();
    let row = stream.into_row().await.unwrap().unwrap();
    let x: Option<i32> = row.try_get("x").unwrap();
    let y: Option<&str> = row.try_get("y").unwrap();
    assert_eq!(x, Some(7));
    assert_eq!(y, Some("ok"));
}

#[tokio::test]
async fn test_mssql_schema_fetch() {
    let url = std::env::var("MSSQL_TEST_URL").ok();
    if url.is_none() { return; }
    let certs = Certificates::new();
    let mut client = mssql::connect::connect(&url.unwrap(), &certs, None, None).await.unwrap();
    let schema = mssql::schema::get_database_schema(&mut client).await.unwrap();
    assert!(schema.schemas.len() >= 1);
}

#[tokio::test]
async fn test_mssql_output_dml() {
    let url = std::env::var("MSSQL_TEST_URL").ok();
    if url.is_none() { return; }
    let certs = Certificates::new();
    let mut client = mssql::connect::connect(&url.unwrap(), &certs, None, None).await.unwrap();

    // Create temp table
    client.simple_query("IF OBJECT_ID('tempdb..#t') IS NOT NULL DROP TABLE #t; CREATE TABLE #t (id INT IDENTITY(1,1) PRIMARY KEY, name NVARCHAR(50))").await.unwrap();

    // INSERT with OUTPUT
    let mut q = tiberius::Query::new("INSERT INTO #t (name) OUTPUT INSERTED.id, INSERTED.name VALUES (@P1), (@P2), (@P3)");
    q.bind("a"); q.bind("b"); q.bind("c");
    let mut s = q.query(&mut client).await.unwrap();
    let mut rows = Vec::new();
    while let Some(item) = s.try_next().await.unwrap() { if let tiberius::QueryItem::Row(r) = item { rows.push(r); } }
    assert_eq!(rows.len(), 3);

    // UPDATE with OUTPUT
    let mut q2 = tiberius::Query::new("UPDATE #t SET name = @P1 OUTPUT INSERTED.id, INSERTED.name WHERE id = 1");
    q2.bind("z");
    let mut s2 = q2.query(&mut client).await.unwrap();
    let r = s2.into_row().await.unwrap().unwrap();
    let id: Option<i32> = r.try_get(0).unwrap();
    let name: Option<&str> = r.try_get(1).unwrap();
    assert_eq!(id, Some(1));
    assert_eq!(name, Some("z"));

    // DELETE with OUTPUT
    let mut q3 = tiberius::Query::new("DELETE FROM #t OUTPUT DELETED.id, DELETED.name WHERE id = 2");
    let mut s3 = q3.query(&mut client).await.unwrap();
    let r3 = s3.into_row().await.unwrap().unwrap();
    let id3: Option<i32> = r3.try_get(0).unwrap();
    let name3: Option<&str> = r3.try_get(1).unwrap();
    assert_eq!(id3, Some(2));
    assert_eq!(name3, Some("b"));
}

#[tokio::test]
async fn test_mssql_paging_offset_fetch() {
    let url = std::env::var("MSSQL_TEST_URL").ok();
    if url.is_none() { return; }
    let certs = Certificates::new();
    let mut client = mssql::connect::connect(&url.unwrap(), &certs, None, None).await.unwrap();

    client.simple_query("IF OBJECT_ID('tempdb..#p') IS NOT NULL DROP TABLE #p; CREATE TABLE #p (id INT PRIMARY KEY)").await.unwrap();
    // Insert 200 rows
    let mut ins = String::from("INSERT INTO #p (id) VALUES ");
    for i in 1..=200 { if i>1 { ins.push_str(","); } ins.push_str(&format!("({})", i)); }
    client.simple_query(&ins).await.unwrap();

    let mut q = tiberius::Query::new("SELECT id FROM #p ORDER BY id OFFSET @P1 ROWS FETCH NEXT @P2 ROWS ONLY");
    q.bind(50i32); q.bind(25i32);
    let mut s = q.query(&mut client).await.unwrap();
    let mut cnt = 0usize;
    while let Some(item) = s.try_next().await.unwrap() { if let tiberius::QueryItem::Row(_) = item { cnt += 1; } }
    assert_eq!(cnt, 25);
}

#[tokio::test]
async fn test_mssql_large_paging() {
    let url = std::env::var("MSSQL_TEST_URL").ok();
    if url.is_none() { return; }
    let certs = Certificates::new();
    let mut client = mssql::connect::connect(&url.unwrap(), &certs, None, None).await.unwrap();

    client.simple_query("IF OBJECT_ID('tempdb..#big') IS NOT NULL DROP TABLE #big; CREATE TABLE #big (id INT PRIMARY KEY)").await.unwrap();
    let mut ins = String::from("DECLARE @i INT = 1; WHILE @i <= 5000 BEGIN INSERT INTO #big (id) VALUES (@i); SET @i = @i + 1; END");
    client.simple_query(&ins).await.unwrap();

    let mut q = tiberius::Query::new("SELECT id FROM #big ORDER BY id OFFSET @P1 ROWS FETCH NEXT @P2 ROWS ONLY");
    q.bind(2500i32); q.bind(100i32);
    let mut s = q.query(&mut client).await.unwrap();
    let mut cnt = 0usize;
    while let Some(item) = s.try_next().await.unwrap() { if let tiberius::QueryItem::Row(_) = item { cnt += 1; } }
    assert_eq!(cnt, 100);
}

#[tokio::test]
async fn test_mssql_cancel_and_reconnect_flow() {
    let url = std::env::var("MSSQL_TEST_URL").ok();
    if url.is_none() { return; }
    let certs = Certificates::new();
    let mut client = mssql::connect::connect(&url.unwrap(), &certs, None, None).await.unwrap();

    // Start a long-running query and cancel by closing the client
    let mut handle_client = client; // take ownership
    let long = tokio::spawn(async move {
        let _ = handle_client.simple_query("WAITFOR DELAY '00:00:05'").await; // expected to be cut short
    });
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    // drop implicitly by not awaiting; reconnect a new client
    let mut client2 = mssql::connect::connect(&url.unwrap(), &certs, None, None).await.unwrap();
    let mut s = client2.simple_query("SELECT 1").await.unwrap();
    let r = s.into_row().await.unwrap().unwrap();
    let v: Option<i32> = r.try_get(0).unwrap();
    assert_eq!(v, Some(1));
    let _ = long.await;
}

#[tokio::test]
async fn test_mssql_schema_pagination() {
    let url = std::env::var("MSSQL_TEST_URL").ok();
    if url.is_none() { return; }
    let certs = Certificates::new();
    let mut client = mssql::connect::connect(&url.unwrap(), &certs, None, None).await.unwrap();
    let mut q = tiberius::Query::new("SELECT TABLE_SCHEMA, TABLE_NAME FROM INFORMATION_SCHEMA.TABLES ORDER BY TABLE_SCHEMA, TABLE_NAME OFFSET @P1 ROWS FETCH NEXT @P2 ROWS ONLY");
    q.bind(0i32); q.bind(10i32);
    let mut s = q.query(&mut client).await.unwrap();
    let mut cnt = 0usize;
    while let Some(item) = s.try_next().await.unwrap() { if let tiberius::QueryItem::Row(_) = item { cnt += 1; } }
    assert_eq!(cnt, 10);
}
