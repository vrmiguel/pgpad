use std::{borrow::Cow, convert::Infallible, path::PathBuf, sync::Arc};

use axum::{
    extract::{rejection::JsonRejection, FromRequest, Path, Query, Request, State},
    http::{header, HeaderMap, StatusCode, Uri},
    middleware::{self, Next},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use pgpad_core::{
    database::{
        services,
        types::{
            ConnectionConfig, ConnectionInfo, DatabaseSchema, Permissions, QuerySnapshot,
            QueryStatus,
        },
    },
    AppState, Certificates, ConnectionMonitor, QueryHistoryEntry,
};
use rand::distr::{Alphanumeric, SampleString};
use rfd::FileDialog;
use rust_embed::Embed;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::value::RawValue;
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};
use uuid::Uuid;

#[derive(Clone)]
pub struct WebState {
    pub app_state: Arc<AppState>,
    pub certificates: Certificates,
    pub connection_monitor: ConnectionMonitor,
    auth_token: String,
}

impl WebState {
    pub fn new(db_path: impl Into<PathBuf>) -> pgpad_core::Result<Self> {
        let app_state = Arc::new(AppState::new(db_path)?);
        let certificates = Certificates::new();
        let (connection_monitor, mut dropped_connections) = ConnectionMonitor::new();

        let state = Self {
            app_state,
            certificates,
            connection_monitor,
            auth_token: generate_auth_token(),
        };

        let certificates = state.certificates.clone();
        tokio::spawn(async move {
            if let Err(e) = certificates.read().await {
                log::warn!("Failed to preload certificates: {e}");
            }
        });

        let app_state = state.app_state.clone();
        tokio::spawn(async move {
            while let Some(connection_id) = dropped_connections.recv().await {
                if !app_state.mark_disconnected(connection_id) {
                    log::error!("Connection {connection_id} not found!");
                    continue;
                }

                log::info!("Connection {connection_id} marked disconnected");
            }
        });

        Ok(state)
    }

    pub fn auth_token(&self) -> &str {
        &self.auth_token
    }
}

fn generate_auth_token() -> String {
    Alphanumeric.sample_string(&mut rand::rng(), 22)
}

pub fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .expect("Failed to get data directory")
        .join("pgpad")
        .join("pgpad.db")
}

pub fn router(state: WebState) -> Router {
    let api = Router::new()
        .route(
            "/commands/initialize_connections",
            post(initialize_connections),
        )
        .route("/commands/get_connections", post(get_connections))
        .route("/commands/get_session_state", post(get_session_state))
        .route("/commands/save_session_state", post(save_session_state))
        .route("/commands/test_connection", post(test_connection))
        .route("/commands/add_connection", post(add_connection))
        .route("/commands/update_connection", post(update_connection))
        .route("/commands/remove_connection", post(remove_connection))
        .route("/commands/connect_to_database", post(connect_to_database))
        .route(
            "/commands/disconnect_from_database",
            post(disconnect_from_database),
        )
        .route("/commands/submit_query", post(submit_query))
        .route(
            "/commands/wait_until_renderable",
            post(wait_until_renderable),
        )
        .route("/commands/fetch_page", post(fetch_page))
        .route("/commands/get_query_status", post(get_query_status))
        .route("/commands/get_page_count", post(get_page_count))
        .route("/commands/is_query_read_only", post(is_query_read_only))
        .route("/commands/get_database_schema", post(get_database_schema))
        .route(
            "/commands/save_query_to_history",
            post(save_query_to_history),
        )
        .route("/commands/export_page", post(export_page))
        .route("/commands/save_script", post(save_script))
        .route("/commands/update_script", post(update_script))
        .route("/commands/get_scripts", post(get_scripts))
        .route("/commands/delete_script", post(delete_script))
        .route("/commands/get_query_history", post(get_query_history))
        .route("/commands/format_sql", post(format_sql))
        .route("/commands/minimize_window", post(noop_command))
        .route("/commands/maximize_window", post(noop_command))
        .route("/commands/close_window", post(noop_command))
        .route("/commands/open_sqlite_db", post(open_sqlite_db))
        .route("/commands/save_sqlite_db", post(save_sqlite_db))
        .route("/commands/pick_ca_cert", post(pick_ca_cert))
        .route("/commands/{command}", post(fallback_command))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_auth_token,
        ));

    Router::new()
        .nest("/api", api)
        .route("/api/events/query", get(query_events))
        .fallback(static_handler)
        .with_state(state)
}

pub fn has_index_html() -> bool {
    Asset::get("index.html").is_some()
}

#[derive(Embed)]
#[folder = "../dist"]
struct Asset;

async fn static_handler(uri: Uri) -> impl IntoResponse {
    StaticFile::from_uri(uri)
}

struct StaticFile {
    path: String,
}

impl StaticFile {
    fn from_uri(uri: Uri) -> Self {
        let path = uri.path().trim_start_matches('/');
        let path = if path.is_empty() { "index.html" } else { path };

        Self {
            path: path.to_string(),
        }
    }

    fn content(&self) -> Option<(Cow<'static, [u8]>, String)> {
        let (content, served_path) = Asset::get(&self.path)
            .map(|file| (file.data, self.path.as_str()))
            .or_else(|| Asset::get("index.html").map(|file| (file.data, "index.html")))?;
        let mime = mime_guess::from_path(served_path).first_or_octet_stream();

        Some((content, mime.to_string()))
    }
}

impl IntoResponse for StaticFile {
    fn into_response(self) -> Response {
        match self.content() {
            Some((content, mime)) => ([(header::CONTENT_TYPE, mime)], content).into_response(),
            None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
        }
    }
}

#[derive(Debug, Serialize)]
struct CommandError {
    message: String,
}

enum CommandHttpError {
    BadRequest(String),
    Core(pgpad_core::Error),
    Join(tokio::task::JoinError),
    NotImplemented(String),
    Serialize(serde_json::Error),
    Unauthorized,
}

impl CommandHttpError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Core(_) | Self::Join(_) | Self::Serialize(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
        }
    }

    fn message(&self) -> String {
        match self {
            Self::BadRequest(message) | Self::NotImplemented(message) => message.clone(),
            Self::Core(error) => error.to_string(),
            Self::Join(error) => error.to_string(),
            Self::Serialize(error) => error.to_string(),
            Self::Unauthorized => "Missing or invalid pgpad-web token".to_string(),
        }
    }
}

impl IntoResponse for CommandHttpError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status_code();
        let error = CommandError {
            message: self.message(),
        };
        (status, Json(error)).into_response()
    }
}

impl From<pgpad_core::Error> for CommandHttpError {
    fn from(error: pgpad_core::Error) -> Self {
        Self::Core(error)
    }
}

impl From<serde_json::Error> for CommandHttpError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialize(error)
    }
}

impl From<tokio::task::JoinError> for CommandHttpError {
    fn from(error: tokio::task::JoinError) -> Self {
        Self::Join(error)
    }
}

impl From<JsonRejection> for CommandHttpError {
    fn from(error: JsonRejection) -> Self {
        Self::BadRequest(error.body_text())
    }
}

async fn require_auth_token(
    State(state): State<WebState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, CommandHttpError> {
    if headers
        .get("x-pgpad-token")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|token| token == state.auth_token())
    {
        Ok(next.run(request).await)
    } else {
        Err(CommandHttpError::Unauthorized)
    }
}

#[derive(Debug, Deserialize)]
struct EventTokenArgs {
    token: String,
}

async fn query_events(
    State(state): State<WebState>,
    Query(EventTokenArgs { token }): Query<EventTokenArgs>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, CommandHttpError> {
    if token != state.auth_token() {
        return Err(CommandHttpError::Unauthorized);
    }

    let query_events_receiver = state.app_state.stmt_manager.query_events_receiver();
    let stream = BroadcastStream::new(query_events_receiver).filter_map(|event| {
        let event = match event {
            Ok(event) => event,
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(skipped)) => {
                log::warn!("Query event stream lagged and skipped {skipped} events");
                return None;
            }
        };

        let data = match serde_json::to_string(&event) {
            Ok(data) => data,
            Err(error) => {
                log::error!("Failed to serialize query event: {error}");
                return None;
            }
        };

        Some(Ok(Event::default().event("query-event").data(data)))
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// `Json<T>` from Axum, but that converts any errors into the Tauri-compatible error structure that the frontend expects
struct CommandJson<T>(T);

impl<S, T> FromRequest<S> for CommandJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = CommandHttpError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state).await?;
        Ok(Self(value))
    }
}

type CommandResult<T> = Result<Json<T>, CommandHttpError>;

async fn initialize_connections(State(state): State<WebState>) -> CommandResult<()> {
    services::initialize_connections(state.app_state.as_ref()).await?;
    Ok(Json(()))
}

async fn get_connections(State(state): State<WebState>) -> CommandResult<Vec<ConnectionInfo>> {
    Ok(Json(
        services::get_connections(state.app_state.as_ref()).await?,
    ))
}

async fn get_session_state(State(state): State<WebState>) -> CommandResult<Option<String>> {
    Ok(Json(
        services::get_session_state(state.app_state.as_ref()).await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveSessionStateArgs {
    session_data: String,
}

async fn save_session_state(
    State(state): State<WebState>,
    CommandJson(SaveSessionStateArgs { session_data }): CommandJson<SaveSessionStateArgs>,
) -> CommandResult<()> {
    services::save_session_state(&session_data, state.app_state.as_ref()).await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestConnectionArgs {
    config: ConnectionConfig,
}

async fn test_connection(
    State(state): State<WebState>,
    CommandJson(TestConnectionArgs { config }): CommandJson<TestConnectionArgs>,
) -> CommandResult<bool> {
    Ok(Json(
        services::test_connection(config, &state.certificates).await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddConnectionArgs {
    name: String,
    config: ConnectionConfig,
    permissions: Permissions,
}

async fn add_connection(
    State(state): State<WebState>,
    CommandJson(AddConnectionArgs {
        name,
        config,
        permissions,
    }): CommandJson<AddConnectionArgs>,
) -> CommandResult<ConnectionInfo> {
    Ok(Json(
        services::add_connection(name, config, permissions, state.app_state.as_ref()).await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateConnectionArgs {
    conn_id: Uuid,
    name: String,
    config: ConnectionConfig,
    permissions: Permissions,
}

async fn update_connection(
    State(state): State<WebState>,
    CommandJson(UpdateConnectionArgs {
        conn_id,
        name,
        config,
        permissions,
    }): CommandJson<UpdateConnectionArgs>,
) -> CommandResult<ConnectionInfo> {
    Ok(Json(
        services::update_connection(conn_id, name, config, permissions, state.app_state.as_ref())
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionIdArgs {
    connection_id: Uuid,
}

async fn remove_connection(
    State(state): State<WebState>,
    CommandJson(ConnectionIdArgs { connection_id }): CommandJson<ConnectionIdArgs>,
) -> CommandResult<()> {
    services::remove_connection(connection_id, state.app_state.as_ref()).await?;
    Ok(Json(()))
}

async fn connect_to_database(
    State(state): State<WebState>,
    CommandJson(ConnectionIdArgs { connection_id }): CommandJson<ConnectionIdArgs>,
) -> CommandResult<bool> {
    Ok(Json(
        services::connect_to_database(
            connection_id,
            state.app_state.as_ref(),
            &state.connection_monitor,
            &state.certificates,
        )
        .await?,
    ))
}

async fn disconnect_from_database(
    State(state): State<WebState>,
    CommandJson(ConnectionIdArgs { connection_id }): CommandJson<ConnectionIdArgs>,
) -> CommandResult<()> {
    services::disconnect_from_database(connection_id, state.app_state.as_ref()).await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitQueryArgs {
    connection_id: Uuid,
    query: String,
}

async fn submit_query(
    State(state): State<WebState>,
    CommandJson(SubmitQueryArgs {
        connection_id,
        query,
    }): CommandJson<SubmitQueryArgs>,
) -> CommandResult<Vec<usize>> {
    Ok(Json(
        services::submit_query(connection_id, &query, state.app_state.as_ref()).await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueryIdArgs {
    query_id: usize,
}

async fn wait_until_renderable(
    State(state): State<WebState>,
    CommandJson(QueryIdArgs { query_id }): CommandJson<QueryIdArgs>,
) -> CommandResult<QuerySnapshot> {
    Ok(Json(
        services::wait_until_renderable(query_id, state.app_state.as_ref()).await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchPageArgs {
    query_id: usize,
    page_index: usize,
}

async fn fetch_page(
    State(state): State<WebState>,
    CommandJson(FetchPageArgs {
        query_id,
        page_index,
    }): CommandJson<FetchPageArgs>,
) -> CommandResult<Option<Box<RawValue>>> {
    Ok(Json(
        services::fetch_page(query_id, page_index, state.app_state.as_ref()).await?,
    ))
}

async fn get_query_status(
    State(state): State<WebState>,
    CommandJson(QueryIdArgs { query_id }): CommandJson<QueryIdArgs>,
) -> CommandResult<QueryStatus> {
    Ok(Json(
        services::get_query_status(query_id, state.app_state.as_ref()).await?,
    ))
}

async fn get_page_count(
    State(state): State<WebState>,
    CommandJson(QueryIdArgs { query_id }): CommandJson<QueryIdArgs>,
) -> CommandResult<usize> {
    Ok(Json(
        services::get_page_count(query_id, state.app_state.as_ref()).await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IsQueryReadOnlyArgs {
    connection_id: Uuid,
    query: String,
}

async fn is_query_read_only(
    State(state): State<WebState>,
    CommandJson(IsQueryReadOnlyArgs {
        connection_id,
        query,
    }): CommandJson<IsQueryReadOnlyArgs>,
) -> CommandResult<bool> {
    Ok(Json(
        services::is_query_read_only(connection_id, &query, state.app_state.as_ref()).await?,
    ))
}

async fn get_database_schema(
    State(state): State<WebState>,
    CommandJson(ConnectionIdArgs { connection_id }): CommandJson<ConnectionIdArgs>,
) -> CommandResult<DatabaseSchema> {
    let schema = services::get_database_schema(connection_id, state.app_state.as_ref()).await?;
    Ok(Json((*schema).clone()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveQueryToHistoryArgs {
    connection_id: String,
    query: String,
    duration_ms: Option<u64>,
    status: String,
    row_count: u64,
    error_message: Option<String>,
}

async fn save_query_to_history(
    State(state): State<WebState>,
    CommandJson(SaveQueryToHistoryArgs {
        connection_id,
        query,
        duration_ms,
        status,
        row_count,
        error_message,
    }): CommandJson<SaveQueryToHistoryArgs>,
) -> CommandResult<()> {
    services::save_query_to_history(
        connection_id,
        query,
        duration_ms,
        status,
        row_count,
        error_message,
        state.app_state.as_ref(),
    )
    .await?;

    Ok(Json(()))
}

async fn export_page(
    State(state): State<WebState>,
    CommandJson(FetchPageArgs {
        query_id,
        page_index,
    }): CommandJson<FetchPageArgs>,
) -> CommandResult<String> {
    Ok(Json(
        services::export_page(query_id, page_index, state.app_state.as_ref()).await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveScriptArgs {
    name: String,
    content: String,
    connection_id: Option<Uuid>,
    description: Option<String>,
}

async fn save_script(
    State(state): State<WebState>,
    CommandJson(SaveScriptArgs {
        name,
        content,
        connection_id,
        description,
    }): CommandJson<SaveScriptArgs>,
) -> CommandResult<i64> {
    Ok(Json(
        services::save_script(
            name,
            content,
            connection_id,
            description,
            state.app_state.as_ref(),
        )
        .await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateScriptArgs {
    id: i64,
    name: String,
    content: String,
    connection_id: Option<Uuid>,
    description: Option<String>,
}

async fn update_script(
    State(state): State<WebState>,
    CommandJson(UpdateScriptArgs {
        id,
        name,
        content,
        connection_id,
        description,
    }): CommandJson<UpdateScriptArgs>,
) -> CommandResult<()> {
    services::update_script(
        id,
        name,
        content,
        connection_id,
        description,
        state.app_state.as_ref(),
    )
    .await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetScriptsArgs {
    connection_id: Option<Uuid>,
}

async fn get_scripts(
    State(state): State<WebState>,
    CommandJson(GetScriptsArgs { connection_id }): CommandJson<GetScriptsArgs>,
) -> CommandResult<Vec<pgpad_core::SavedQuery>> {
    Ok(Json(
        services::get_scripts(connection_id, state.app_state.as_ref()).await?,
    ))
}

#[derive(Debug, Deserialize)]
struct DeleteScriptArgs {
    id: i64,
}

async fn delete_script(
    State(state): State<WebState>,
    CommandJson(DeleteScriptArgs { id }): CommandJson<DeleteScriptArgs>,
) -> CommandResult<()> {
    services::delete_script(id, state.app_state.as_ref()).await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetQueryHistoryArgs {
    connection_id: String,
    limit: Option<u32>,
}

async fn get_query_history(
    State(state): State<WebState>,
    CommandJson(GetQueryHistoryArgs {
        connection_id,
        limit,
    }): CommandJson<GetQueryHistoryArgs>,
) -> CommandResult<Vec<QueryHistoryEntry>> {
    Ok(Json(
        services::get_query_history(connection_id, limit, state.app_state.as_ref()).await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FormatSqlArgs {
    query: String,
}

async fn format_sql(
    CommandJson(FormatSqlArgs { query }): CommandJson<FormatSqlArgs>,
) -> CommandResult<String> {
    Ok(Json(services::format_sql(&query).await?))
}

async fn noop_command() -> CommandResult<()> {
    Ok(Json(()))
}

async fn run_file_dialog<F>(dialog: F) -> Result<Option<String>, CommandHttpError>
where
    F: FnOnce() -> Option<PathBuf> + Send + 'static,
{
    Ok(tokio::task::spawn_blocking(dialog)
        .await?
        .map(|path| path.to_string_lossy().into_owned()))
}

async fn open_sqlite_db() -> CommandResult<Option<String>> {
    Ok(Json(
        run_file_dialog(|| {
            FileDialog::new()
                .set_title("Pick a SQLite database file")
                .add_filter("SQLite database", &["db", "sqlite", "sqlite3"])
                .pick_file()
        })
        .await?,
    ))
}

async fn save_sqlite_db() -> CommandResult<Option<String>> {
    Ok(Json(
        run_file_dialog(|| {
            FileDialog::new()
                .set_title("Create a new SQLite database file")
                .save_file()
        })
        .await?,
    ))
}

async fn pick_ca_cert() -> CommandResult<Option<String>> {
    Ok(Json(
        run_file_dialog(|| {
            FileDialog::new()
                .set_title("Pick a certificate file")
                .add_filter("Certificate files", &["pem", "crt", "cer", "ca-bundle"])
                .pick_file()
        })
        .await?,
    ))
}

async fn fallback_command(Path(command): Path<String>) -> Result<Json<()>, CommandHttpError> {
    Err(CommandHttpError::NotImplemented(format!(
        "Command '{command}' is not implemented yet"
    )))
}
