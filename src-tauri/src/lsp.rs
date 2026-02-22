use std::sync::{mpsc::channel, Arc};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Event, Listener, Manager};
use uuid::Uuid;

use crate::{database::types::DatabaseSchema, AppState, Error, Result};

#[derive(Debug, Deserialize)]
struct Request {
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct Response {
    jsonrpc: &'static str,
    id: Option<Value>,
    result: Option<Value>,
    error: Option<LspError>,
}

#[derive(Debug, Serialize)]
struct LspError {
    code: i32,
    message: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CompletionItem {
    label: String,
    kind: u32,
    detail: Option<String>,
    documentation: Option<String>,
    insert_text: Option<String>,
    filter_text: Option<String>,
    sort_text: Option<String>,
}

mod completion_kind {
    pub const CLASS: u32 = 7;
    pub const MODULE: u32 = 9;
    pub const FIELD: u32 = 5;
    pub const KEYWORD: u32 = 14;
}

const SQL_KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "INSERT INTO",
    "UPDATE",
    "DELETE FROM",
    "ORDER BY",
    "GROUP BY",
    "JOIN",
    "LEFT JOIN",
    "INNER JOIN",
    "RIGHT JOIN",
    "OUTER JOIN",
    "UNION",
    "UNION ALL",
    "CREATE TABLE",
    "DROP TABLE",
    "HAVING",
    "DISTINCT",
    "COUNT",
    "SUM",
    "AVG",
    "MAX",
    "MIN",
    "AND",
    "OR",
    "NOT",
    "IN",
    "LIKE",
    "BETWEEN",
    "IS NULL",
    "IS NOT NULL",
    "AS",
    "LIMIT",
    "OFFSET",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
];

pub fn setup_listener(app: AppHandle) {
    let (tx, rx) = channel::<Event>();

    app.listen("lsp-request", move |event| {
        if let Err(err) = tx.send(event) {
            log::error!("LSP listener channel closed: {}", err);
        }
    });

    tauri::async_runtime::spawn_blocking(move || {
        let mut server = LanguageServer::new(app.clone());
        while let Ok(event) = rx.recv() {
            if let Err(err) = server.handle_event(event) {
                log::error!("Failed to handle LSP event: {}", err);
            }
        }
    });
}

struct LanguageServer {
    app: AppHandle,
    connection_id: Option<Uuid>,
}

impl LanguageServer {
    fn new(app: AppHandle) -> Self {
        Self {
            app,
            connection_id: None,
        }
    }

    fn handle_event(&mut self, event: Event) -> Result {
        let raw_payload = parse_payload(event.payload())?;
        let request: Request =
            serde_json::from_str(&raw_payload).context("Failed to parse LSP request payload")?;

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "textDocument/completion" => self.handle_completion(request.id),
            "pgpad/connectionSelected" => self.handle_connection_selected(request.params),
            // We accept but ignore document sync notifications for now.
            "textDocument/didOpen" | "textDocument/didChange" | "textDocument/didClose" => Ok(()),
            _ => {
                if request.id.is_some() {
                    self.send_response(request.id, Some(serde_json::Value::Null), None)?;
                }
                Ok(())
            }
        }
    }

    fn handle_initialize(&self, id: Option<Value>) -> Result {
        let result = serde_json::json!({
            "capabilities": {
                "completionProvider": {
                    "triggerCharacters": [".", " ", "\n", "\t", ","]
                },
                "textDocumentSync": {
                    "openClose": true,
                    "change": 2
                }
            },
            "serverInfo": {
                "name": "pgpad-lsp",
                "version": "0.1.0"
            }
        });

        self.send_response(id, Some(result), None)
    }

    fn handle_completion(&self, id: Option<Value>) -> Result {
        let mut items = self.keyword_completions();
        items.extend(self.schema_completions());

        let result = serde_json::json!({
            "isIncomplete": false,
            "items": items
        });

        self.send_response(id, Some(result), None)
    }

    fn handle_connection_selected(&mut self, params: Option<Value>) -> Result {
        self.connection_id = match params {
            Some(Value::String(connection_id)) => match Uuid::parse_str(&connection_id) {
                Ok(uuid) => Some(uuid),
                Err(err) => {
                    log::warn!(
                        "LSP received invalid connection id '{}': {}",
                        connection_id,
                        err
                    );
                    None
                }
            },
            Some(Value::Null) | None => None,
            Some(other) => {
                log::warn!("LSP received unsupported connection payload: {}", other);
                None
            }
        };

        Ok(())
    }

    fn send_response(
        &self,
        id: Option<Value>,
        result: Option<Value>,
        error: Option<LspError>,
    ) -> Result {
        let response = Response {
            jsonrpc: "2.0",
            id,
            result,
            error,
        };

        let payload =
            serde_json::to_string(&response).context("Failed to serialize LSP response")?;
        self.app
            .emit("lsp-response", payload)
            .context("Failed to emit LSP response")
            .map_err(Error::from)
    }

    fn keyword_completions(&self) -> Vec<CompletionItem> {
        SQL_KEYWORDS
            .iter()
            .map(|keyword| CompletionItem {
                label: (*keyword).to_string(),
                kind: completion_kind::KEYWORD,
                detail: Some("SQL keyword".to_string()),
                documentation: Some(format!("SQL keyword: {}", keyword)),
                insert_text: Some(format!("{} ", keyword)),
                filter_text: Some((*keyword).to_string()),
                sort_text: Some(format!("0_{}", keyword)),
            })
            .collect()
    }

    fn schema_completions(&self) -> Vec<CompletionItem> {
        let Some(schema) = self.get_schema() else {
            return Vec::new();
        };

        let mut completions = Vec::new();

        for table in &schema.tables {
            let table_name = if table.schema.is_empty() || table.schema == "public" {
                table.name.clone()
            } else {
                format!("{}.{}", table.schema, table.name)
            };

            completions.push(CompletionItem {
                label: format_identifier(&table_name),
                kind: completion_kind::CLASS,
                detail: Some("table".to_string()),
                documentation: Some(format!(
                    "Table: {} ({} columns)",
                    table_name,
                    table.columns.len()
                )),
                insert_text: Some(format_identifier(&table_name)),
                filter_text: Some(table_name.clone()),
                sort_text: Some(format!("1_{}", table_name)),
            });

            for column in &table.columns {
                let qualified_name = format!("{}.{}", table_name, column.name);
                completions.push(CompletionItem {
                    label: format_identifier(&qualified_name),
                    kind: completion_kind::FIELD,
                    detail: Some(format!("{} column", table_name)),
                    documentation: Some(format!("Column: {} ({})", column.name, column.data_type)),
                    insert_text: Some(format_identifier(&qualified_name)),
                    filter_text: Some(qualified_name.clone()),
                    sort_text: Some(format!("2_{}", qualified_name)),
                });
            }
        }

        for column_name in &schema.unique_columns {
            completions.push(CompletionItem {
                label: format_identifier(column_name),
                kind: completion_kind::FIELD,
                detail: Some("column".to_string()),
                documentation: Some(format!("Column: {}", column_name)),
                insert_text: Some(format_identifier(column_name)),
                filter_text: Some(column_name.clone()),
                sort_text: Some(format!("3_{}", column_name)),
            });
        }

        for schema_name in &schema.schemas {
            if schema_name == "public" {
                continue;
            }

            completions.push(CompletionItem {
                label: format_identifier(schema_name),
                kind: completion_kind::MODULE,
                detail: Some("schema".to_string()),
                documentation: Some(format!("Schema: {}", schema_name)),
                insert_text: Some(format_identifier(schema_name)),
                filter_text: Some(schema_name.clone()),
                sort_text: Some(format!("4_{}", schema_name)),
            });
        }

        completions
    }

    fn get_schema(&self) -> Option<Arc<DatabaseSchema>> {
        let connection_id = self.connection_id?;
        let state = self.app.state::<AppState>();
        state
            .schemas
            .get(&connection_id)
            .map(|schema| schema.clone())
    }
}

fn parse_payload(payload: &str) -> Result<String> {
    let trimmed = payload.trim();
    if trimmed.starts_with('"') {
        let unescaped: String =
            serde_json::from_str(trimmed).context("Failed to unescape payload")?;
        Ok(unescaped)
    } else {
        Ok(trimmed.to_string())
    }
}

fn needs_quoting(identifier: &str) -> bool {
    if identifier.chars().any(|c| c.is_uppercase()) {
        return true;
    }
    if identifier.chars().any(|c| !c.is_alphanumeric() && c != '_') {
        return true;
    }

    identifier
        .chars()
        .next()
        .map(|c| c.is_numeric())
        .unwrap_or(false)
}

fn format_identifier(identifier: &str) -> String {
    if identifier.contains('.') {
        return identifier
            .split('.')
            .map(|part| {
                if needs_quoting(part) {
                    format!("\"{}\"", part)
                } else {
                    part.to_string()
                }
            })
            .collect::<Vec<String>>()
            .join(".");
    }

    if needs_quoting(identifier) {
        format!("\"{}\"", identifier)
    } else {
        identifier.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{format_identifier, needs_quoting, parse_payload};

    #[test]
    fn parse_payload_accepts_plain_and_escaped_json() {
        let plain = r#"{"jsonrpc":"2.0","method":"initialize"}"#;
        assert_eq!(parse_payload(plain).unwrap(), plain);

        let escaped = serde_json::to_string(plain).unwrap();
        assert_eq!(parse_payload(&escaped).unwrap(), plain);
    }

    #[test]
    fn identifier_formatting_quotes_only_when_needed() {
        assert!(!needs_quoting("users"));
        assert!(needs_quoting("User"));
        assert!(needs_quoting("my-table"));
        assert!(needs_quoting("123abc"));

        assert_eq!(format_identifier("users"), "users");
        assert_eq!(format_identifier("User"), "\"User\"");
        assert_eq!(format_identifier("public.User"), "public.\"User\"");
        assert_eq!(format_identifier("my-schema.table"), "\"my-schema\".table");
    }
}
