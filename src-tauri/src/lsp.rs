use std::{
    sync::{mpsc::channel, Arc},
    time::Instant,
};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Event, Listener, Manager};
use uuid::Uuid;

use crate::{
    database::types::DatabaseSchema,
    lsp::parser::{ParsingContext, SuggestionCategory, SuggestionType},
    AppState, Error,
};
use tree_sitter::Point;

mod parser;

/// A request from CodeMirror
#[derive(Debug, Deserialize)]
pub struct Request {
    pub id: Option<serde_json::Value>,
    pub method: RequestMethod,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub enum RequestMethod {
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
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    pub completion_provider: Option<CompletionOptions>,
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

#[derive(Debug, Deserialize)]
pub struct CompletionParams {
    pub position: Position,
    pub context: Option<CompletionContext>,
}

#[derive(Debug, Deserialize)]
pub struct Position {
    pub character: u32,
    pub line: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionContext {
    pub trigger_character: Option<String>,
    #[allow(dead_code)]
    pub trigger_kind: u32,
}

pub fn setup_listener(app: AppHandle) {
    let (tx, rx) = channel();

    app.listen("lsp-request", move |event| {
        if let Err(err) = tx.send(event) {
            log::error!("LSP listener channel closed: {}", err);
        }
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
    parsing_context: ParsingContext,
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
            parsing_context: ParsingContext::new().unwrap(),
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
        let message = parse_payload(event.payload())?;

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

    fn handle_completion(&mut self, request: Request) -> Result<(), Error> {
        let now = Instant::now();

        let params = request
            .params
            .as_ref()
            .ok_or_else(|| Error::Any(anyhow::anyhow!("No params in completion request")))?;

        let completion_params: CompletionParams = serde_json::from_value(params.clone())?;
        log::info!(
            "Handling completion request, params: {:?}",
            completion_params
        );

        let completion_items = match self.handle_completion_with_parser(&completion_params) {
            Ok(items) => items,
            Err(e) => {
                log::warn!(
                    "Parser-based completion failed, falling back to basic completion: {}",
                    e
                );
                self.handle_completion_fallback(&completion_params)?
            }
        };

        let result = CompletionList {
            is_incomplete: false,
            items: completion_items,
        };

        let elapsed = now.elapsed();
        log::info!("Completion request handled in {:?}ms", elapsed.as_millis());

        self.send_response(request.id, Some(serde_json::to_value(result)?), None)
    }

    fn handle_completion_with_parser(
        &mut self,
        params: &CompletionParams,
    ) -> Result<Vec<CompletionItem>, Error> {
        let position = Point {
            row: params.position.line as usize,
            column: params.position.character as usize,
        };

        log::debug!("Completion request at position: {:?}", position);

        // Use the sophisticated parser to get suggestion type
        let suggestion_type = self
            .parsing_context
            .suggestion_for_position(position)
            .map_err(|e| Error::Any(anyhow::anyhow!("Parser completion failed: {}", e)))?;

        log::info!("Parser suggests: {:?}", suggestion_type);
        log::info!("Current parser state: {:?}", self.parsing_context.state);
        log::info!("Current token: {:?}", self.parsing_context.current_token);

        // Convert parser suggestions to LSP completion items
        Ok(self.convert_suggestions_to_completions(suggestion_type))
    }

    fn handle_completion_fallback(
        &self,
        params: &CompletionParams,
    ) -> Result<Vec<CompletionItem>, Error> {
        let mut completion_items = Vec::new();

        for keyword in SQL_KEYWORDS {
            if self.should_include_keyword(&keyword.keyword, params) {
                completion_items.push(CompletionItem {
                    label: keyword.keyword.to_string(),
                    kind: completion_kind::KEYWORD,
                    detail: Some("SQL keyword".to_string()),
                    documentation: Some(format!("SQL keyword: {}", keyword.keyword)),
                    insert_text: Some(format!("{} ", keyword.keyword)),
                    filter_text: Some(keyword.keyword.to_string()),
                    sort_text: Some(format!("0_{}", keyword.keyword)),
                });
            }
        }

        let schema_completions = self.schema_completions_for_context(params);
        completion_items.extend(schema_completions.cloned());

        Ok(completion_items)
    }

    fn convert_suggestions_to_completions(
        &self,
        suggestion_type: SuggestionType,
    ) -> Vec<CompletionItem> {
        // Get current partial text for keyword filtering
        let partial_text = self
            .parsing_context
            .current_token
            .as_ref()
            .map(|token| token.text.as_str());

        // Get tables from parser context
        let available_tables: Vec<String> = self
            .parsing_context
            .scope
            .available_tables
            .keys()
            .cloned()
            .collect();

        match suggestion_type {
            SuggestionType::Keywords => self.get_filtered_keyword_completions(partial_text),
            SuggestionType::Tables => self.get_table_completions(),
            SuggestionType::Columns { tables } => {
                // Use tables from parser context if available, otherwise use the provided tables
                let table_list = if tables.is_empty() {
                    available_tables
                } else {
                    tables
                };
                self.get_column_completions(table_list)
            }
            SuggestionType::Functions => self.get_function_completions(),
            SuggestionType::Values(values) => self.get_value_completions(values),
            SuggestionType::Mixed(categories) => {
                let mut items = Vec::new();
                for category in categories {
                    items.extend(self.convert_suggestion_category_to_completions(
                        category,
                        partial_text,
                        &available_tables,
                    ));
                }
                items
            }
        }
    }

    fn convert_suggestion_category_to_completions(
        &self,
        category: SuggestionCategory,
        partial_text: Option<&str>,
        available_tables: &[String],
    ) -> Vec<CompletionItem> {
        match category {
            SuggestionCategory::Keywords => self.get_filtered_keyword_completions(partial_text),
            SuggestionCategory::Tables => self.get_table_completions(),
            SuggestionCategory::Columns { tables } => {
                // Use tables from parser context if available, otherwise use the provided tables
                let table_list = if tables.is_empty() {
                    available_tables.to_vec()
                } else {
                    tables
                };
                self.get_column_completions(table_list)
            }
            SuggestionCategory::Functions => self.get_function_completions(),
            SuggestionCategory::Values(values) => self.get_value_completions(values),
        }
    }

    fn get_filtered_keyword_completions(&self, partial_text: Option<&str>) -> Vec<CompletionItem> {
        SQL_KEYWORDS
            .iter()
            .filter(|keyword| {
                if let Some(partial) = partial_text {
                    if partial.is_empty() {
                        return true;
                    }
                    // Case-insensitive prefix matching for partial keywords
                    keyword
                        .keyword
                        .to_lowercase()
                        .starts_with(&partial.to_lowercase())
                } else {
                    true
                }
            })
            .map(|keyword| CompletionItem {
                label: keyword.keyword.to_string(),
                kind: completion_kind::KEYWORD,
                detail: Some("SQL keyword".to_string()),
                documentation: Some(format!("SQL keyword: {}", keyword.keyword)),
                insert_text: Some(keyword.keyword.to_string()), // Don't add space for partial completions
                filter_text: Some(keyword.keyword.to_string()),
                sort_text: Some(format!("0_{}", keyword.keyword)),
            })
            .collect()
    }

    fn get_table_completions(&self) -> Vec<CompletionItem> {
        self.schema_completions
            .iter()
            .filter(|item| item.kind == completion_kind::CLASS)
            .cloned()
            .collect()
    }

    fn get_column_completions(&self, tables: Vec<String>) -> Vec<CompletionItem> {
        log::info!("Getting column completions for tables: {:?}", tables);
        if tables.is_empty() {
            // Return all column completions
            self.schema_completions
                .iter()
                .filter(|item| item.kind == completion_kind::FIELD)
                .cloned()
                .collect()
        } else {
            // Return columns for specific tables
            self.schema_completions
                .iter()
                .filter(|item| {
                    item.kind == completion_kind::FIELD
                        && item.filter_text.as_ref().map_or(false, |filter_text| {
                            tables.iter().any(|table| filter_text.starts_with(table))
                        })
                })
                .cloned()
                .collect()
        }
    }

    fn get_function_completions(&self) -> Vec<CompletionItem> {
        // For now, return function-like SQL keywords
        SQL_KEYWORDS
            .iter()
            .filter(|keyword| matches!(keyword.keyword, "COUNT" | "SUM" | "AVG" | "MAX" | "MIN"))
            .map(|keyword| CompletionItem {
                label: keyword.keyword.to_string(),
                kind: completion_kind::FUNCTION,
                detail: Some("SQL function".to_string()),
                documentation: Some(format!("SQL function: {}", keyword.keyword)),
                insert_text: Some(format!("{}(", keyword.keyword)),
                filter_text: Some(keyword.keyword.to_string()),
                sort_text: Some(format!("1_{}", keyword.keyword)),
            })
            .collect()
    }

    fn get_value_completions(&self, values: Vec<String>) -> Vec<CompletionItem> {
        values
            .into_iter()
            .map(|value| CompletionItem {
                label: value.clone(),
                kind: completion_kind::VALUE,
                detail: Some("Value".to_string()),
                documentation: Some(format!("Value: {}", value)),
                insert_text: Some(value.clone()),
                filter_text: Some(value.clone()),
                sort_text: Some(format!("2_{}", value)),
            })
            .collect()
    }

    fn should_include_keyword(&self, keyword: &str, params: &CompletionParams) -> bool {
        // Context-specific filtering based on trigger character
        if let Some(context) = &params.context {
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
        } else {
            true // Include all keywords by default when no context
        }
    }

    fn schema_completions_for_context<'a>(
        &'a self,
        _params: &CompletionParams,
    ) -> impl Iterator<Item = &'a CompletionItem> + 'a {
        // For now, return all schema completions
        // TODO: Add filtering based on context when needed
        self.schema_completions.iter()
    }

    fn handle_did_open(&mut self, request: Request) -> Result<(), Error> {
        log::info!("Handling didOpen notification");

        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct DidOpenParams {
            text_document: TextDocument,
        }

        #[derive(Deserialize, Debug)]
        struct TextDocument {
            text: String,
        }

        if let Some(params) = &request.params {
            let params = serde_json::from_value::<DidOpenParams>(params.clone())?;
            self.parsing_context.did_change(params.text_document.text)?;
        }

        Ok(())
    }

    fn handle_did_change(&mut self, request: Request) -> Result<(), Error> {
        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct DidChangeParams {
            content_changes: Vec<ContentChange>,
        }
        // TODO(vini): handle when the client sends just the diff
        #[derive(Deserialize, Debug)]
        struct ContentChange {
            text: String,
        }

        log::info!("Handling didChange notification");
        if let Some(params) = &request.params {
            let params = serde_json::from_value::<DidChangeParams>(params.clone())?;
            // log::info!("Deserialized didChange: {:?}", params);
            for content_change in params.content_changes {
                self.parsing_context.did_change(content_change.text)?;
            }
        }

        Ok(())
    }

    fn handle_connection_selected(&mut self, request: Request) -> Result<(), Error> {
        match request.params {
            Some(Value::String(connection_id)) => match Uuid::parse_str(&connection_id) {
                Ok(uuid) => {
                    self.connection_id = Some(uuid);
                    self.generate_schema_completions();
                }
                Err(err) => {
                    log::warn!(
                        "LSP received invalid connection id '{}': {}",
                        connection_id,
                        err
                    );
                    self.connection_id = None;
                    self.schema_completions.clear();
                }
            },
            Some(Value::Null) | None => {
                self.connection_id = None;
                self.schema_completions.clear();
            }
            Some(other) => {
                log::warn!("LSP received unsupported connection payload: {}", other);
                self.connection_id = None;
                self.schema_completions.clear();
            }
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

        self.app
            .emit("lsp-response", response_str)
            .context("Failed to emit LSP response")
            .map_err(Into::into)
    }
}

fn parse_payload(payload: &str) -> Result<String, Error> {
    let trimmed = payload.trim();
    if trimmed.starts_with('"') {
        let unescaped: String =
            serde_json::from_str(trimmed).context("Failed to unescape payload")?;
        Ok(unescaped)
    } else {
        Ok(trimmed.to_string())
    }
}
