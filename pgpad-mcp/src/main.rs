use std::{path::PathBuf, sync::Arc};

use pgpad_core::{
    database::{
        services,
        types::{ConnectionConfig, ConnectionInfo},
    },
    AppState, Certificates, ConnectionMonitor,
};
use rmcp::{
    handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool, tool_handler,
    tool_router, transport::stdio, ErrorData as McpError, ServerHandler, ServiceExt,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

type McpResult = Result<CallToolResult, McpError>;

#[derive(Clone)]
struct Pgpad {
    state: Arc<AppState>,
    certs: Certificates,
    monitor: ConnectionMonitor,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ConnectionId {
    connection_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct QueryArgs {
    connection_id: String,
    query: String,
}

#[tool_router]
impl Pgpad {
    async fn new() -> anyhow::Result<Self> {
        let state = Arc::new(AppState::new(db_path())?);
        services::initialize_connections(&state).await?;

        let (monitor, mut dropped) = ConnectionMonitor::new();
        let dropped_state = state.clone();
        tokio::spawn(async move {
            while let Some(id) = dropped.recv().await {
                dropped_state.mark_disconnected(id);
            }
        });

        Ok(Self {
            state,
            certs: Certificates::new(),
            monitor,
        })
    }

    #[tool(description = "List saved databases")]
    async fn list_connections(&self) -> McpResult {
        let connections = services::get_connections(&self.state)
            .await
            .map_err(mcp_error)?;
        ok(connections
            .into_iter()
            .map(redact_connection)
            .collect::<Vec<_>>())
    }

    #[tool(description = "Return the database schema for a database")]
    async fn schema(&self, Parameters(args): Parameters<ConnectionId>) -> McpResult {
        let id = parse_uuid(&args.connection_id)?;
        self.ensure_connected(id).await?;

        let schema = services::get_database_schema(id, &self.state)
            .await
            .map_err(mcp_error)?;
        ok(&*schema)
    }

    #[tool(description = "Disconnect from a database")]
    async fn disconnect(&self, Parameters(args): Parameters<ConnectionId>) -> McpResult {
        let id = parse_uuid(&args.connection_id)?;
        services::disconnect_from_database(id, &self.state)
            .await
            .map_err(mcp_error)?;
        ok(json!({ "disconnected": true }))
    }

    #[tool(description = "Run a read-only SQL query and return its first result page")]
    async fn query(&self, Parameters(args): Parameters<QueryArgs>) -> McpResult {
        let id = parse_uuid(&args.connection_id)?;
        self.ensure_connected(id).await?;

        if !services::is_query_read_only(id, &args.query, &self.state)
            .await
            .map_err(mcp_error)?
        {
            return Err(McpError::invalid_params("query must be read-only", None));
        }

        let ids = services::submit_query(id, &args.query, &self.state)
            .await
            .map_err(mcp_error)?;
        let mut snapshots = Vec::new();
        for query_id in &ids {
            snapshots.push(
                services::wait_until_renderable(*query_id, &self.state)
                    .await
                    .map_err(mcp_error)?,
            );
        }
        services::release_queries(&ids, &self.state)
            .await
            .map_err(mcp_error)?;

        ok(json!({ "query_ids": ids, "snapshots": snapshots }))
    }

    async fn ensure_connected(&self, id: Uuid) -> Result<(), McpError> {
        if self
            .state
            .connections
            .get(&id)
            .is_some_and(|connection| connection.is_client_connected())
        {
            return Ok(());
        }

        if services::connect_to_database(id, &self.state, &self.monitor, &self.certs)
            .await
            .map_err(mcp_error)?
        {
            Ok(())
        } else {
            Err(McpError::internal_error("connection failed", None))
        }
    }
}

#[tool_handler(name = "pgpad-mcp", version = "0.1.0")]
impl ServerHandler for Pgpad {}

fn db_path() -> PathBuf {
    std::env::var_os("PGPAD_MCP_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_dir()
                .expect("data dir not found")
                .join("pgpad")
                .join("pgpad.db")
        })
}

fn parse_uuid(value: &str) -> Result<Uuid, McpError> {
    value
        .parse()
        .map_err(|_| McpError::invalid_params(format!("invalid connection id: {value}"), None))
}

fn ok(value: impl serde::Serialize) -> McpResult {
    serde_json::to_value(value)
        .map(CallToolResult::structured)
        .map_err(mcp_error)
}

fn mcp_error(error: impl std::fmt::Display) -> McpError {
    McpError::internal_error(error.to_string(), None)
}

fn redact_connection(connection: ConnectionInfo) -> serde_json::Value {
    let kind = match connection.config {
        ConnectionConfig::Postgres { .. } => "postgres",
        ConnectionConfig::SQLite { .. } => "sqlite",
    };

    json!({
        "id": connection.id,
        "name": connection.name,
        "connected": connection.connected,
        "permissions": connection.permissions,
        "kind": kind,
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Pgpad::new().await?.serve(stdio()).await?.waiting().await?;
    Ok(())
}
