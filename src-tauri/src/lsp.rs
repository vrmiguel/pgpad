use std::{
    sync::{mpsc::channel, Arc},
    time::Instant,
};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Event, Listener, Manager};
use uuid::Uuid;

use crate::{database::types::DatabaseSchema, AppState, Error};

/// A request from CodeMirror
#[derive(Debug, Deserialize)]
struct Request {
    pub id: Option<serde_json::Value>,
    pub method: RequestMethod,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
enum RequestMethod {
    #[serde(rename = "initialize")]
    Initialize,
    #[serde(rename = "textDocument/completion")]
    Completion,
    #[serde(rename = "textDocument/didOpen")]
    DidOpen,
    #[serde(rename = "textDocument/didChange")]
    DidChange,
    #[serde(rename = "textDocument/didClose")]
    DidClose,
    #[serde(rename = "pgpad/connectionSelected")]
    ConnectionSelected,
    #[serde(other)]
    Other,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub error: Option<LSPError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LSPError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    pub label: String,
    pub kind: u32,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: Option<String>,
    pub filter_text: Option<String>,
    pub sort_text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompletionList {
    #[serde(rename = "isIncomplete")]
    pub is_incomplete: bool,
    pub items: Vec<CompletionItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(rename = "completionProvider")]
    pub completion_provider: Option<CompletionOptions>,
    #[serde(rename = "textDocumentSync")]
    pub text_document_sync: Option<TextDocumentSyncOptions>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompletionOptions {
    #[serde(rename = "triggerCharacters")]
    pub trigger_characters: Option<Vec<String>>,
    #[serde(rename = "resolveProvider")]
    pub resolve_provider: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextDocumentSyncOptions {
    #[serde(rename = "openClose")]
    pub open_close: Option<bool>,
    pub change: Option<u32>,
    #[serde(rename = "willSave")]
    pub will_save: Option<bool>,
    #[serde(rename = "willSaveWaitUntil")]
    pub will_save_wait_until: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: Option<ServerInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[allow(dead_code)]
pub mod completion_kind {
    pub const TEXT: u32 = 1;
    pub const METHOD: u32 = 2;
    pub const FUNCTION: u32 = 3;
    pub const CONSTRUCTOR: u32 = 4;
    pub const FIELD: u32 = 5;
    pub const VARIABLE: u32 = 6;
    pub const CLASS: u32 = 7;
    pub const INTERFACE: u32 = 8;
    pub const MODULE: u32 = 9;
    pub const PROPERTY: u32 = 10;
    pub const UNIT: u32 = 11;
    pub const VALUE: u32 = 12;
    pub const ENUM: u32 = 13;
    pub const KEYWORD: u32 = 14;
    pub const SNIPPET: u32 = 15;
    pub const COLOR: u32 = 16;
    pub const FILE: u32 = 17;
    pub const REFERENCE: u32 = 18;
    pub const FOLDER: u32 = 19;
    pub const ENUM_MEMBER: u32 = 20;
    pub const CONSTANT: u32 = 21;
    pub const STRUCT: u32 = 22;
    pub const EVENT: u32 = 23;
    pub const OPERATOR: u32 = 24;
    pub const TYPE_PARAMETER: u32 = 25;
}

#[derive(Debug, Default)]
struct CompletionContext {
    line: u32,
    character: u32,
    typed_text: String,
    explicit_trigger: bool,
    trigger_character: Option<String>,
    document_uri: String,
}

pub fn setup_listener(app: AppHandle) {
    let (tx, rx) = channel();

    app.listen("lsp-request", move |event| {
        tx.send(event).unwrap();
    });

    let app_clone = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut language_server = LanguageServer::new(app_clone.clone());
        while let Ok(event) = rx.recv() {
            if let Err(e) = language_server.lsp_request(event) {
                log::error!("Failed to handle LSP request: {}", e);
            }
        }
    });
}

struct LanguageServer {
    app: AppHandle,
    // Currently selected connection
    connection_id: Option<Uuid>,
    // Completions for the current selection
    schema_completions: Vec<CompletionItem>,
}

#[derive(Clone)]
struct SqlKeyword {
    keyword: &'static str,
}

const SQL_KEYWORDS: &[SqlKeyword] = &[
    SqlKeyword { keyword: "SELECT" },
    SqlKeyword { keyword: "FROM" },
    SqlKeyword { keyword: "WHERE" },
    SqlKeyword {
        keyword: "INSERT INTO",
    },
    SqlKeyword { keyword: "UPDATE" },
    SqlKeyword {
        keyword: "DELETE FROM",
    },
    SqlKeyword {
        keyword: "ORDER BY",
    },
    SqlKeyword {
        keyword: "GROUP BY",
    },
    SqlKeyword { keyword: "JOIN" },
    SqlKeyword {
        keyword: "LEFT JOIN",
    },
    SqlKeyword {
        keyword: "INNER JOIN",
    },
    SqlKeyword {
        keyword: "RIGHT JOIN",
    },
    SqlKeyword {
        keyword: "OUTER JOIN",
    },
    SqlKeyword { keyword: "UNION" },
    SqlKeyword {
        keyword: "UNION ALL",
    },
    SqlKeyword {
        keyword: "CREATE TABLE",
    },
    SqlKeyword {
        keyword: "DROP TABLE",
    },
    SqlKeyword { keyword: "HAVING" },
    SqlKeyword {
        keyword: "DISTINCT",
    },
    SqlKeyword { keyword: "COUNT" },
    SqlKeyword { keyword: "SUM" },
    SqlKeyword { keyword: "AVG" },
    SqlKeyword { keyword: "MAX" },
    SqlKeyword { keyword: "MIN" },
    SqlKeyword { keyword: "AND" },
    SqlKeyword { keyword: "OR" },
    SqlKeyword { keyword: "NOT" },
    SqlKeyword { keyword: "IN" },
    SqlKeyword { keyword: "LIKE" },
    SqlKeyword { keyword: "BETWEEN" },
    SqlKeyword { keyword: "IS NULL" },
    SqlKeyword {
        keyword: "IS NOT NULL",
    },
    SqlKeyword { keyword: "AS" },
    SqlKeyword { keyword: "LIMIT" },
    SqlKeyword { keyword: "OFFSET" },
    SqlKeyword { keyword: "CASE" },
    SqlKeyword { keyword: "WHEN" },
    SqlKeyword { keyword: "THEN" },
    SqlKeyword { keyword: "ELSE" },
    SqlKeyword { keyword: "END" },
];

impl LanguageServer {
    fn new(app: AppHandle) -> Self {
        Self {
            app,
            connection_id: None,
            schema_completions: Vec::new(),
        }
    }

    // TODO(vini): is this applicable to Postgres only?
    fn needs_quoting(identifier: &str) -> bool {
        if identifier.chars().any(|c| c.is_uppercase()) {
            return true;
        }
        if identifier.chars().any(|c| !c.is_alphanumeric() && c != '_') {
            return true;
        }
        
        if identifier.chars().next().map_or(false, |c| c.is_numeric()) {
            return true;
        }
        false
    }

    fn format_identifier(identifier: &str) -> String {
        if identifier.contains('.') {
            // Qualified identifiers (table.column)
            let parts: Vec<&str> = identifier.split('.').collect();
            let formatted_parts: Vec<String> = parts
                .iter()
                .map(|part| {
                    if Self::needs_quoting(part) {
                        format!("\"{}\"", part)
                    } else {
                        part.to_string()
                    }
                })
                .collect();
            formatted_parts.join(".")
        } else if Self::needs_quoting(identifier) {
            format!("\"{}\"", identifier)
        } else {
            identifier.to_string()
        }
    }

    /// Get the schema for the current connection
    fn get_schema(&self) -> Option<Arc<DatabaseSchema>> {
        self.connection_id.and_then(|connection_id| {
            let state = self.app.state::<AppState>();
            state
                .schemas
                .get(&connection_id)
                .map(|schema| schema.clone())
        })
    }

    fn generate_schema_completions(&mut self) {
        self.schema_completions.clear();

        if let Some(schema) = &self.get_schema() {
            for table in &schema.tables {
                let table_name = if table.schema.is_empty() || table.schema == "public" {
                    table.name.clone()
                } else {
                    format!("{}.{}", table.schema, table.name)
                };

                self.schema_completions.push(CompletionItem {
                    label: Self::format_identifier(&table_name),
                    kind: completion_kind::CLASS, // Class (for tables)
                    detail: Some("table".to_string()),
                    documentation: Some(format!(
                        "Table: {} ({} columns)",
                        table_name,
                        table.columns.len()
                    )),
                    insert_text: Some(Self::format_identifier(&table_name)),
                    filter_text: Some(table_name.clone()),
                    sort_text: Some(format!("1_{}", table_name)),
                });

                for column in &table.columns {
                    let qualified_name = format!("{}.{}", table_name, column.name);
                    let nullable_text = if column.is_nullable {
                        ", nullable"
                    } else {
                        ", not null"
                    };

                    self.schema_completions.push(CompletionItem {
                        label: Self::format_identifier(&qualified_name),
                        kind: completion_kind::FIELD,
                        detail: Some(format!("{} column", table_name)),
                        documentation: Some(format!(
                            "Column: {} ({}) from {}{}",
                            column.name, column.data_type, table_name, nullable_text
                        )),
                        insert_text: Some(Self::format_identifier(&qualified_name)),
                        filter_text: Some(qualified_name.clone()),
                        sort_text: Some(format!("3_{}", qualified_name)),
                    });
                }
            }

            for column_name in &schema.unique_columns {
                self.schema_completions.push(CompletionItem {
                    label: Self::format_identifier(column_name),
                    kind: completion_kind::FIELD,
                    detail: Some("column".to_string()),
                    documentation: Some(format!("Column: {}", column_name)),
                    insert_text: Some(Self::format_identifier(column_name)),
                    filter_text: Some(column_name.clone()),
                    sort_text: Some(format!("2_{}", column_name)),
                });
            }

            // Entries with non-public schemas
            for schema_name in &schema.schemas {
                if schema_name != "public" {
                    self.schema_completions.push(CompletionItem {
                        label: Self::format_identifier(schema_name),
                        kind: completion_kind::MODULE,
                        detail: Some("schema".to_string()),
                        documentation: Some(format!("Schema: {}", schema_name)),
                        insert_text: Some(Self::format_identifier(schema_name)),
                        filter_text: Some(schema_name.clone()),
                        sort_text: Some(format!("4_{}", schema_name)),
                    });
                }
            }
        }
    }

    fn lsp_request(&mut self, event: Event) -> Result<(), Error> {
        // The message arrives annoyingly escaped, requires unescaping
        let message = serde_json::from_str::<String>(event.payload())?;

        let json = serde_json::from_str::<Value>(&message)?;
        println!("LSP request: {}", serde_json::to_string_pretty(&json)?);

        match serde_json::from_str::<Request>(&message) {
            Ok(request) => {
                match request.method {
                    RequestMethod::Initialize => self.handle_initialize(request),
                    RequestMethod::Completion => self.handle_completion(request),
                    RequestMethod::DidOpen => self.handle_did_open(request),
                    RequestMethod::DidChange => self.handle_did_change(request),
                    RequestMethod::DidClose => {
                        /* todo */
                        Ok(())
                    }
                    RequestMethod::ConnectionSelected => self.handle_connection_selected(request),
                    RequestMethod::Other => {
                        log::info!("Unhandled LSP method. Request: {:?}", request);
                        self.send_response(request.id, None, None)
                    }
                }
            }
            Err(e) => {
                log::info!("Failed to parse LSP request: {}", e);
                log::info!("Raw message: {}", message);
                Err(Error::Any(anyhow::anyhow!("Invalid LSP request: {}", e)))
            }
        }
    }

    fn handle_initialize(&self, request: Request) -> Result<(), Error> {
        log::info!("Handling initialize request");

        let capabilities = ServerCapabilities {
            completion_provider: Some(CompletionOptions {
                trigger_characters: Some(vec![
                    ".".to_string(),
                    " ".to_string(),
                    "(".to_string(),
                    ",".to_string(),
                    "\n".to_string(),
                    "\t".to_string(),
                ]),
                resolve_provider: Some(false),
            }),
            text_document_sync: Some(TextDocumentSyncOptions {
                open_close: Some(true),
                // Ask for incremental changes.
                // The client will still not do this for text under 1024 characters.
                change: Some(2),
                will_save: Some(false),
                will_save_wait_until: Some(false),
            }),
        };

        let result = InitializeResult {
            capabilities,
            server_info: Some(ServerInfo {
                name: "pgpad-lsp".to_string(),
                version: "0.1.0".to_string(),
            }),
        };

        self.send_response(request.id, Some(serde_json::to_value(result)?), None)
    }

    fn handle_completion(&self, request: Request) -> Result<(), Error> {
        log::info!("Handling completion request: {:?}", request);
        let now = Instant::now();

        let params = request.params.as_ref();
        let context = self.extract_completion_context(params);

        let mut completion_items = Vec::new();

        for keyword in SQL_KEYWORDS {            
            if self.should_include_keyword(&keyword.keyword, &context) {
                completion_items.push(CompletionItem {
                    label: keyword.keyword.to_string(),
                    kind: completion_kind::KEYWORD,
                    detail: Some("SQL keyword".to_string()),
                    documentation: Some(format!("SQL keyword: {}", keyword.keyword)),
                    insert_text: Some(format!("{} ", keyword.keyword)),
                    // What the CodeMirror LSP actually uses for autocompletion filtering
                    filter_text: Some(keyword.keyword.to_string()),
                    // How the CodeMirror LSP orders completions
                    sort_text: Some(format!("0_{}", keyword.keyword)),
                });
            }
        }

        let schema_completions = self.schema_completions_for_context(&context);
        completion_items.extend(schema_completions.cloned());

        let is_incomplete = context.typed_text.len() < 2 && !context.explicit_trigger;

        let result = CompletionList {
            is_incomplete,
            items: completion_items,
        };

        let elapsed = now.elapsed();
        log::info!("Completion request handled in {:?}ms", elapsed.as_millis());

        self.send_response(request.id, Some(serde_json::to_value(result)?), None)
    }

    fn extract_completion_context(&self, params: Option<&serde_json::Value>) -> CompletionContext {
        let mut context = CompletionContext::default();

        if let Some(params) = params {
            if let Some(position) = params.get("position") {
                context.line = position.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                context.character = position
                    .get("character")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
            }

            if let Some(trigger_ctx) = params.get("context") {
                context.explicit_trigger = trigger_ctx
                    .get("triggerKind")
                    .and_then(|v| v.as_u64())
                    .map(|k| k == 2) // TriggerKind.Invoked
                    .unwrap_or(false);

                context.trigger_character = trigger_ctx
                    .get("triggerCharacter")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }

            // Extract text document context for more advanced parsing
            if let Some(text_doc) = params.get("textDocument") {
                if let Some(uri) = text_doc.get("uri").and_then(|v| v.as_str()) {
                    context.document_uri = uri.to_string();
                }
            }
        }

        context
    }

    fn should_include_keyword(&self, keyword: &str, context: &CompletionContext) -> bool {
        if !context.typed_text.is_empty() {
            let keyword_lower = keyword.to_lowercase();
            let typed_lower = context.typed_text.to_lowercase();

            // Only include if keyword starts with typed text (prefix match)
            if !keyword_lower.starts_with(&typed_lower) {
                return false;
            }
        }

        // Context-specific filtering
        match context.trigger_character.as_deref() {
            Some(".") => {
                // After dot, prefer column/field keywords
                matches!(keyword, "COUNT" | "SUM" | "AVG" | "MAX" | "MIN")
            }
            Some(",") => {
                // After comma, prefer column names and some keywords
                !matches!(keyword, "CREATE TABLE" | "DROP TABLE")
            }
            Some("(") => {
                // After opening paren, prefer function-like keywords
                matches!(
                    keyword,
                    "SELECT" | "COUNT" | "SUM" | "AVG" | "MAX" | "MIN" | "DISTINCT"
                )
            }
            _ => true, // Include all keywords by default
        }
    }

    fn schema_completions_for_context<'a, 'b>(
        &'a self,
        context: &'b CompletionContext,
    ) -> impl Iterator<Item = &'a CompletionItem> + 'a 
    where
        'b: 'a,
    {
        self.schema_completions.iter().filter(move |completion| {
            if let Some(filter_text) = &completion.filter_text {
                // If we have typed text, apply prefix filtering
                if !context.typed_text.is_empty() {
                    let filter_lower = filter_text.to_lowercase();
                    let typed_lower = context.typed_text.to_lowercase();

                    // Check prefix match or contains match for longer queries
                    filter_lower.starts_with(&typed_lower)
                        || (typed_lower.len() > 2 && filter_lower.contains(&typed_lower))
                } else {
                    true
                }
            } else {
                true
            }
        })
    }

    fn handle_did_open(&self, request: Request) -> Result<(), Error> {
        log::info!("Handling didOpen notification");
        if let Some(params) = &request.params {
            if let Some(text_document) = params.get("textDocument") {
                log::info!("Document opened: {:?}", text_document);
            }
        }

        Ok(())
    }

    fn handle_did_change(&self, request: Request) -> Result<(), Error> {
        log::info!("Handling didChange notification");
        if let Some(params) = &request.params {
            // log::info!(
            //     "Document changes: {}",
            //     serde_json::to_string_pretty(params)?
            // );
        }

        Ok(())
    }

    fn handle_connection_selected(&mut self, request: Request) -> Result<(), Error> {
        log::info!("Handling updateSchema request");

        if let Some(params) = &request.params {
            match serde_json::from_value::<Uuid>(params.clone()) {
                Ok(schema) => {
                    self.connection_id = Some(schema);
                    self.generate_schema_completions();
                }
                Err(err) => {
                    log::error!("LSP: Failed to parse schema: {err}");
                }
            }
        } else {
            log::error!("LSP: Missing schema parameters");
        }

        Ok(())
    }

    fn send_response(
        &self,
        id: Option<serde_json::Value>,
        result: Option<serde_json::Value>,
        error: Option<LSPError>,
    ) -> Result<(), Error> {
        let response = Response {
            jsonrpc: "2.0".to_string(),
            id,
            result,
            error,
        };

        let response_str =
            serde_json::to_string(&response).context("Failed to serialize response")?;

        // log::info!("Sending LSP response: {}", response_str);

        self.app
            .emit("lsp-response", response_str)
            .context("Failed to emit LSP response")
            .map_err(Into::into)
    }
}
