use axum::{
    body::{Body, Bytes},
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

fn make_static_dir() -> TempDir {
    let dir = tempfile::tempdir().expect("failed to create static dir");
    std::fs::write(
        dir.path().join("index.html"),
        "<!doctype html><div id=\"app\"></div>",
    )
    .expect("failed to write index.html");
    dir
}

fn make_sqlite_db() -> TempDir {
    let dir = tempfile::tempdir().expect("failed to create sqlite dir");
    let db_path = dir.path().join("user.sqlite");
    let conn = rusqlite::Connection::open(&db_path).expect("failed to open sqlite db");

    conn.execute_batch(
        r#"
        CREATE TABLE items (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        );

        INSERT INTO items (name) VALUES ('alpha'), ('beta');
        "#,
    )
    .expect("failed to seed sqlite db");

    dir
}

async fn command(app: Router, name: &str, payload: Value) -> (StatusCode, Bytes) {
    let request = Request::post(format!("/api/commands/{name}"))
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .expect("failed to build request");

    let response = app.oneshot(request).await.expect("request failed");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("failed to collect body")
        .to_bytes();

    (status, body)
}

async fn command_ok<T>(app: Router, name: &str, payload: Value) -> T
where
    T: DeserializeOwned,
{
    let (status, body) = command(app, name, payload).await;
    assert_eq!(status, StatusCode::OK, "body: {}", body_text(&body));
    serde_json::from_slice(&body).expect("failed to deserialize response")
}

fn body_text(body: &Bytes) -> String {
    String::from_utf8_lossy(body).into_owned()
}

#[tokio::test]
async fn sqlite_connection_query_flow_works_over_http_commands() {
    let static_dir = make_static_dir();
    let sqlite_dir = make_sqlite_db();
    let app_dir = tempfile::tempdir().expect("failed to create app dir");
    let app_db = app_dir.path().join("pgpad.db");
    let user_db = sqlite_dir.path().join("user.sqlite");

    let state = pgpad_web::WebState::new(app_db).expect("failed to create web state");
    let app = pgpad_web::router(static_dir.path().to_path_buf(), state);

    let _: Value = command_ok(app.clone(), "initialize_connections", json!({})).await;

    let connection: Value = command_ok(
        app.clone(),
        "add_connection",
        json!({
            "name": "Smoke SQLite",
            "config": {
                "SQLite": {
                    "db_path": user_db
                }
            },
            "permissions": "read_write"
        }),
    )
    .await;

    let connection_id = connection
        .get("id")
        .and_then(Value::as_str)
        .expect("connection id missing");

    let connections: Vec<Value> = command_ok(app.clone(), "get_connections", json!({})).await;
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0]["connected"], false);

    let connected: bool = command_ok(
        app.clone(),
        "connect_to_database",
        json!({ "connectionId": connection_id }),
    )
    .await;
    assert!(connected);

    let connections: Vec<Value> = command_ok(app.clone(), "get_connections", json!({})).await;
    assert_eq!(connections[0]["connected"], true);

    let schema: Value = command_ok(
        app.clone(),
        "get_database_schema",
        json!({ "connectionId": connection_id }),
    )
    .await;
    assert_eq!(schema["tables"][0]["name"], "items");

    let read_only: bool = command_ok(
        app.clone(),
        "is_query_read_only",
        json!({
            "connectionId": connection_id,
            "query": "SELECT id, name FROM items ORDER BY id"
        }),
    )
    .await;
    assert!(read_only);

    let query_ids: Vec<usize> = command_ok(
        app.clone(),
        "submit_query",
        json!({
            "connectionId": connection_id,
            "query": "SELECT id, name FROM items ORDER BY id"
        }),
    )
    .await;
    assert_eq!(query_ids, vec![0]);

    let snapshot: Value = command_ok(
        app.clone(),
        "wait_until_renderable",
        json!({ "queryId": query_ids[0] }),
    )
    .await;
    assert_eq!(snapshot["status"], "Completed");
    assert_eq!(snapshot["columns"], json!(["id", "name"]));
    assert_eq!(snapshot["first_page"], json!([[1, "alpha"], [2, "beta"]]));

    let status: String = command_ok(
        app.clone(),
        "get_query_status",
        json!({ "queryId": query_ids[0] }),
    )
    .await;
    assert_eq!(status, "Completed");

    let page_count: usize = command_ok(
        app.clone(),
        "get_page_count",
        json!({ "queryId": query_ids[0] }),
    )
    .await;
    assert_eq!(page_count, 1);

    let page: Value = command_ok(
        app.clone(),
        "fetch_page",
        json!({ "queryId": query_ids[0], "pageIndex": 0 }),
    )
    .await;
    assert_eq!(page, json!([[1, "alpha"], [2, "beta"]]));

    let csv: String = command_ok(
        app.clone(),
        "export_page",
        json!({ "queryId": query_ids[0], "pageIndex": 0 }),
    )
    .await;
    assert_eq!(csv, "id,name\n1,\"alpha\"\n2,\"beta\"\n");

    let _: Value = command_ok(
        app.clone(),
        "save_query_to_history",
        json!({
            "connectionId": connection_id,
            "query": "SELECT id, name FROM items ORDER BY id",
            "durationMs": 5,
            "status": "success",
            "rowCount": 2,
            "errorMessage": null
        }),
    )
    .await;

    let history: Vec<Value> = command_ok(
        app.clone(),
        "get_query_history",
        json!({ "connectionId": connection_id, "limit": 10 }),
    )
    .await;
    assert_eq!(history.len(), 1);
    assert_eq!(
        history[0]["query_text"],
        "SELECT id, name FROM items ORDER BY id"
    );

    let _: Value = command_ok(
        app.clone(),
        "disconnect_from_database",
        json!({ "connectionId": connection_id }),
    )
    .await;

    let connections: Vec<Value> = command_ok(app.clone(), "get_connections", json!({})).await;
    assert_eq!(connections[0]["connected"], false);

    let (status, body) = command(app, "not_real", json!({})).await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    let error: Value = serde_json::from_slice(&body).expect("failed to parse error body");
    assert_eq!(
        error["message"],
        "Command 'not_real' is not implemented yet"
    );
}
