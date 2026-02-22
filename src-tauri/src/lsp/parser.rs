#![allow(dead_code)]

use std::collections::HashMap;
use tree_sitter::{Node, Parser, Point, Query, QueryCursor, StreamingIterator, Tree};

/// Main parsing context that tracks the current state and provides suggestions
pub struct ParsingContext {
    /// The current query text
    pub query: String,

    /// Current parsing state at the cursor
    pub state: ParseState,

    /// Schema information (tables, columns, functions)
    pub schema: SchemaInfo,

    /// Current query scope (what tables/columns are available)
    pub scope: QueryScope,

    /// The token currently being typed (if any)
    pub current_token: Option<PartialToken>,

    /// Tree-sitter parser for SQL
    parser: Parser,

    /// Current parse tree
    current_tree: Option<Tree>,
}

/// Represents the current parsing state and what we're expecting next
#[derive(Debug, Clone, PartialEq)]
pub enum ParseState {
    /// Initial state - any statement type possible
    Initial,

    /// Currently typing a keyword (e.g., "SEL" -> "SELECT")
    TypingKeyword {
        partial: String,
        candidates: Vec<String>,
    },

    /// In SELECT clause - expecting columns, functions, or FROM
    InSelectClause {
        items: Vec<SelectItem>,
        expecting: SelectExpectation,
    },

    /// In FROM clause - expecting table names or joins
    InFromClause {
        tables: Vec<TableReference>,
        expecting: FromExpectation,
    },

    /// In WHERE clause - expecting column references and conditions
    InWhereClause {
        conditions: Vec<Condition>,
        expecting: WhereExpectation,
    },

    /// In ORDER BY clause - expecting column references
    InOrderByClause {
        items: Vec<OrderByItem>,
        expecting: OrderByExpectation,
    },

    /// Other clauses (GROUP BY, HAVING, LIMIT, etc.)
    InOtherClause {
        clause_type: ClauseType,
        expecting: GenericExpectation,
    },
}

/// What we're expecting in different SELECT contexts
#[derive(Debug, Clone, PartialEq)]
pub enum SelectExpectation {
    /// Expecting column name, function, or *
    ColumnOrExpression,
    /// Expecting comma or FROM keyword
    CommaOrFrom,
    /// Expecting FROM keyword specifically
    FromKeyword,
}

/// What we're expecting in FROM contexts
#[derive(Debug, Clone, PartialEq)]
pub enum FromExpectation {
    /// Expecting table name or subquery
    TableName,
    /// Expecting table alias
    TableAlias,
    /// Expecting JOIN keyword or WHERE/ORDER BY/etc
    JoinOrClause,
    /// Expecting subsequent clauses (WHERE, ORDER BY, etc.)
    SubsequentClause,
}

/// What we're expecting in WHERE contexts
#[derive(Debug, Clone, PartialEq)]
pub enum WhereExpectation {
    /// Expecting column reference
    ColumnReference,
    /// Expecting operator (=, >, <, etc.)
    Operator,
    /// Expecting value or parameter
    Value,
    /// Expecting AND/OR
    LogicalOperator,
}

/// What we're expecting in ORDER BY contexts
#[derive(Debug, Clone, PartialEq)]
pub enum OrderByExpectation {
    /// Expecting column name
    ColumnName,
    /// Expecting ASC/DESC or comma
    DirectionOrComma,
}

/// Generic expectation for other clause types
#[derive(Debug, Clone, PartialEq)]
pub enum GenericExpectation {
    Keyword,
    Expression,
    Value,
}

/// Types of SQL clauses
#[derive(Debug, Clone, PartialEq)]
pub enum ClauseType {
    GroupBy,
    Having,
    Limit,
    Offset,
}

/// Represents a partial token being typed
#[derive(Debug, Clone)]
pub struct PartialToken {
    pub text: String,
    pub start_position: usize,
    pub end_position: usize,
}

/// Schema information for suggestions
#[derive(Debug, Clone)]
pub struct SchemaInfo {
    pub tables: HashMap<String, TableInfo>,
    pub functions: Vec<FunctionInfo>,
    pub keywords: Vec<String>,
    pub data_types: Vec<String>,
}

/// Information about a database table
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub schema: Option<String>,
    pub columns: Vec<ColumnInfo>,
    pub table_type: TableType,
}

/// Information about a table column
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub is_foreign_key: bool,
}

/// Information about available functions
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub return_type: String,
    pub parameters: Vec<ParameterInfo>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub name: String,
    pub data_type: String,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TableType {
    Table,
    View,
    MaterializedView,
}

/// Current query scope - what's available for completion
#[derive(Debug, Clone)]
pub struct QueryScope {
    /// Tables available in current scope (alias -> table_name)
    pub available_tables: HashMap<String, String>,
    /// Columns available from tables in scope
    pub available_columns: Vec<ScopedColumn>,
    /// Available aliases
    pub aliases: HashMap<String, AliasInfo>,
}

/// Column that's available in current scope
#[derive(Debug, Clone)]
pub struct ScopedColumn {
    pub table_name: String,
    pub table_alias: Option<String>,
    pub column_name: String,
    pub data_type: String,
    pub fully_qualified_name: String, // e.g., "users.id" or "u.id"
}

/// Information about an alias
#[derive(Debug, Clone)]
pub struct AliasInfo {
    pub alias: String,
    pub target: AliasTarget,
}

#[derive(Debug, Clone)]
pub enum AliasTarget {
    Table(String),
    Column(String),
    Expression(String),
}

// Parsed query components
#[derive(Debug, Clone, PartialEq)]
pub struct SelectItem {
    pub expression: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableReference {
    pub name: String,
    pub alias: Option<String>,
    pub schema: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub left: String,
    pub operator: String,
    pub right: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderByItem {
    pub column: String,
    pub direction: Option<SortDirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// Types of suggestions that can be provided
///
/// This enum tells the LSP server what kind of suggestions should be prioritized
/// based on the current SQL parsing context. The LSP server will use its schema
/// knowledge to provide the actual suggestions.
#[derive(Debug, Clone, PartialEq)]
pub enum SuggestionType {
    /// Suggest SQL keywords
    Keywords,
    /// Suggest table names
    Tables,
    /// Suggest columns for the specified tables
    Columns {
        /// Suggest columns for these tables (empty means all available tables)
        tables: Vec<String>,
    },
    /// Suggest function names
    Functions,
    /// Suggest specific literal values
    Values(Vec<String>),
    /// Mixed suggestions (multiple types are appropriate)
    Mixed(Vec<SuggestionCategory>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SuggestionCategory {
    Keywords,
    Tables,
    Columns { tables: Vec<String> },
    Functions,
    Values(Vec<String>),
}

impl ParsingContext {
    /// Create a new SQL parsing context
    pub fn new() -> Result<Self, String> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_sequel::LANGUAGE.into())
            .map_err(|_| "Failed to set SQL language for parser")?;

        Ok(ParsingContext {
            query: String::new(),
            state: ParseState::Initial,
            schema: SchemaInfo {
                tables: HashMap::new(),
                functions: Vec::new(),
                keywords: vec![
                    "SELECT".to_string(),
                    "FROM".to_string(),
                    "WHERE".to_string(),
                    "ORDER BY".to_string(),
                    "GROUP BY".to_string(),
                    "HAVING".to_string(),
                    "INSERT".to_string(),
                    "UPDATE".to_string(),
                    "DELETE".to_string(),
                    "JOIN".to_string(),
                    "INNER JOIN".to_string(),
                    "LEFT JOIN".to_string(),
                    "RIGHT JOIN".to_string(),
                    "FULL JOIN".to_string(),
                ],
                data_types: vec![
                    "INTEGER".to_string(),
                    "VARCHAR".to_string(),
                    "TEXT".to_string(),
                    "BOOLEAN".to_string(),
                    "DATE".to_string(),
                    "TIMESTAMP".to_string(),
                ],
            },
            scope: QueryScope {
                available_tables: HashMap::new(),
                available_columns: Vec::new(),
                aliases: HashMap::new(),
            },
            current_token: None,
            parser,
            current_tree: None,
        })
    }

    /// Handle a `textDocument/didChange` event from the LSP client
    ///
    /// This updates the internal document state and re-parses the content
    /// using incremental parsing when possible.
    pub fn did_change(&mut self, new_text: String) -> crate::Result<()> {
        // Check if the text actually changed
        if self.query == new_text {
            return Ok(());
        }

        self.query = new_text;

        if let Some(tree) = self.parser.parse(&self.query, self.current_tree.as_ref()) {
            self.current_tree = Some(tree);
            Ok(())
        } else {
            Err(crate::Error::Any(anyhow::anyhow!(
                "Failed to parse SQL document"
            )))
        }
    }

    /// Handle a `textDocument/completion` request from the LSP client
    ///
    /// Returns completion suggestions based on the cursor position and current document state.
    pub fn suggestion_for_position(&mut self, position: Point) -> Result<SuggestionType, String> {
        // Check if we have a valid tree for the current query
        let need_reparse = match &self.current_tree {
            Some(tree) => {
                let tree_text_len = tree.root_node().end_byte();
                let query_len = self.query.len();
                tree_text_len != query_len // Tree is stale if lengths don't match
            }
            None => {
                true // No tree at all
            }
        };

        if need_reparse {
            if let Some(tree) = self.parser.parse(&self.query, self.current_tree.as_ref()) {
                self.current_tree = Some(tree.clone());
            } else {
                self.current_tree = None;
                self.handle_parse_failure_at_position(position);
                return Ok(self.get_suggestion_type());
            }
        }

        // Now we should have a valid tree
        if let Some(tree) = self.current_tree.clone() {
            // Extract current token at cursor
            self.current_token = self.extract_current_token(position);

            // Analyze the parse tree to determine current state
            self.state = self.determine_parse_state(position);

            // Update query scope with available tables and columns
            self.update_query_scope(&tree);

            // Return suggestions based on current state
            Ok(self.get_suggestion_type())
        } else {
            // This shouldn't happen, but handle it gracefully
            self.handle_parse_failure_at_position(position);
            Ok(self.get_suggestion_type())
        }
    }

    /// Get the type of suggestions that should be provided based on current context
    pub fn get_suggestion_type(&self) -> SuggestionType {
        match &self.state {
            ParseState::Initial => SuggestionType::Keywords,

            ParseState::TypingKeyword { .. } => SuggestionType::Keywords,

            ParseState::InSelectClause { expecting, .. } => {
                match expecting {
                    SelectExpectation::ColumnOrExpression => {
                        // In SELECT, we want columns, functions, and some keywords like *
                        SuggestionType::Mixed(vec![
                            SuggestionCategory::Columns {
                                tables: self.get_available_table_names(),
                            },
                            SuggestionCategory::Functions,
                            SuggestionCategory::Keywords, // for *, CASE, etc.
                        ])
                    }
                    SelectExpectation::CommaOrFrom | SelectExpectation::FromKeyword => {
                        SuggestionType::Keywords
                    }
                }
            }

            ParseState::InFromClause { expecting, .. } => match expecting {
                FromExpectation::TableName => SuggestionType::Tables,
                FromExpectation::SubsequentClause => SuggestionType::Keywords,
                _ => SuggestionType::Keywords,
            },

            ParseState::InWhereClause { expecting, .. } => {
                match expecting {
                    WhereExpectation::ColumnReference => SuggestionType::Columns {
                        tables: self.get_available_table_names(),
                    },
                    WhereExpectation::Operator => SuggestionType::Keywords,
                    WhereExpectation::Value => SuggestionType::Values(vec![]), // LSP will provide actual values
                    WhereExpectation::LogicalOperator => SuggestionType::Keywords,
                }
            }

            ParseState::InOrderByClause { expecting, .. } => match expecting {
                OrderByExpectation::ColumnName => SuggestionType::Columns {
                    tables: self.get_available_table_names(),
                },
                OrderByExpectation::DirectionOrComma => SuggestionType::Keywords,
            },

            ParseState::InOtherClause { .. } => SuggestionType::Keywords,
        }
    }

    /// Get the names of tables currently available in scope
    fn get_available_table_names(&self) -> Vec<String> {
        self.scope.available_tables.keys().cloned().collect()
    }

    fn handle_parse_failure_at_position(&mut self, position: Point) {
        // When parsing fails, try to provide basic keyword completion
        if let Some(last_word) = self.get_last_word_at_position(position) {
            if self.looks_like_keyword(&last_word) {
                self.state = ParseState::TypingKeyword {
                    partial: last_word.clone(),
                    candidates: self.get_keyword_candidates(&last_word),
                };

                self.current_token = Some(PartialToken {
                    text: last_word.clone(),
                    start_position: 0, // We'll calculate this properly if needed
                    end_position: 0,
                });
            } else {
                self.state = ParseState::Initial;
            }
        } else {
            self.state = ParseState::Initial;
        }
    }
    /// Convert Point to byte position for text operations
    fn point_to_byte(&self, point: Point) -> usize {
        let mut byte_pos = 0;
        let mut current_row = 0;
        let mut current_col = 0;

        for ch in self.query.chars() {
            if current_row == point.row && current_col == point.column {
                break;
            }

            if ch == '\n' {
                current_row += 1;
                current_col = 0;
            } else {
                current_col += 1;
            }

            byte_pos += ch.len_utf8();
        }

        byte_pos
    }

    /// Extract the current token being typed at the cursor position
    fn extract_current_token(&self, cursor_point: Point) -> Option<PartialToken> {
        let cursor_byte = self.point_to_byte(cursor_point);
        let text_up_to_cursor = &self.query[..cursor_byte.min(self.query.len())];

        // Find the last word before the cursor
        let mut word_start = None;

        // Look backwards from cursor to find word boundaries
        for (i, ch) in text_up_to_cursor.char_indices().rev() {
            if ch.is_whitespace() || ch == '(' || ch == ')' || ch == ',' || ch == ';' {
                word_start = Some(i + ch.len_utf8());
                break;
            }
        }

        let start = word_start.unwrap_or(0);

        // If we're at the end of a word or there's no current word being typed
        if start >= text_up_to_cursor.len() {
            return None;
        }

        let current_word = text_up_to_cursor[start..].to_string();

        // Only return a token if we're actually typing something
        if current_word.trim().is_empty() {
            return None;
        }

        Some(PartialToken {
            text: current_word.trim().to_string(),
            start_position: start,
            end_position: cursor_byte,
        })
    }

    /// Find the deepest node at the given point
    fn find_node_at_point<'a>(&self, tree: &'a Tree, point: Point) -> Option<Node<'a>> {
        let mut cursor = tree.walk();

        loop {
            let current_node = cursor.node();

            // Check if point is within current node
            if !self.point_in_node(&current_node, point) {
                return None;
            }

            // Try to find a child that contains the point
            if cursor.goto_first_child() {
                let mut found_child = false;
                loop {
                    let child_node = cursor.node();
                    if self.point_in_node(&child_node, point) {
                        found_child = true;
                        break;
                    }

                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }

                if !found_child {
                    cursor.goto_parent();
                    return Some(cursor.node());
                }
            } else {
                // No children, this is the deepest node
                return Some(current_node);
            }
        }
    }

    /// Check if a point is within a node
    fn point_in_node(&self, node: &Node, point: Point) -> bool {
        point.row >= node.start_position().row
            && point.row <= node.end_position().row
            && (point.row != node.start_position().row
                || point.column >= node.start_position().column)
            && (point.row != node.end_position().row || point.column <= node.end_position().column)
    }

    /// Check if a node has a child of a specific kind
    fn node_has_child_of_kind(&self, node: Node, kind: &str) -> bool {
        let mut cursor = node.walk();

        if cursor.goto_first_child() {
            loop {
                if cursor.node().kind() == kind {
                    return true;
                }

                // Also check grandchildren
                if self.node_has_child_of_kind(cursor.node(), kind) {
                    return true;
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        false
    }

    /// Determine the current parse state based on tree analysis
    fn determine_parse_state(&self, cursor_point: Point) -> ParseState {
        let Some(tree) = self.current_tree.as_ref() else {
            return ParseState::Initial;
        };
        // For incomplete queries, use text-based analysis combined with tree structure
        let state = self.analyze_query_text_context(cursor_point);

        // If text-based analysis gives us a specific state, use it
        if !matches!(state, ParseState::Initial) {
            return state;
        }

        // Fall back to tree-based analysis
        if let Some(node) = self.find_containing_clause(tree, cursor_point) {
            match node.kind() {
                "select" => self.analyze_select_state(&node, cursor_point),
                "from" => self.analyze_from_state(&node, cursor_point),
                "where" => self.analyze_where_state(&node, cursor_point),
                "order_by" => self.analyze_order_by_state(&node, cursor_point),
                "statement" => {
                    // For statement nodes, we need to look deeper
                    self.analyze_statement_node(&node, cursor_point)
                }
                _ => {
                    // Check if we're typing a keyword
                    if let Some(token) = &self.current_token {
                        if self.looks_like_keyword(&token.text) {
                            return ParseState::TypingKeyword {
                                partial: token.text.clone(),
                                candidates: self.get_keyword_candidates(&token.text),
                            };
                        }
                    }
                    ParseState::Initial
                }
            }
        } else {
            // No containing clause found - might be at the beginning
            if let Some(token) = &self.current_token {
                if self.looks_like_keyword(&token.text) {
                    return ParseState::TypingKeyword {
                        partial: token.text.clone(),
                        candidates: self.get_keyword_candidates(&token.text),
                    };
                }
            }
            ParseState::Initial
        }
    }

    /// Analyze query text to determine context (for incomplete queries)
    fn analyze_query_text_context(&self, cursor_point: Point) -> ParseState {
        let cursor_byte = self.point_to_byte(cursor_point);
        let text_up_to_cursor = &self.query[..cursor_byte.min(self.query.len())];
        let text_lower = text_up_to_cursor.to_lowercase();

        // Check for WHERE clause context
        if text_lower.contains("where") {
            // Find the position of the last WHERE
            if let Some(where_pos) = text_lower.rfind("where") {
                let after_where = &text_up_to_cursor[where_pos + 5..].trim_start();

                if after_where.is_empty() {
                    // Right after WHERE keyword
                    return ParseState::InWhereClause {
                        conditions: Vec::new(),
                        expecting: WhereExpectation::ColumnReference,
                    };
                } else {
                    // In the middle of WHERE conditions
                    return ParseState::InWhereClause {
                        conditions: Vec::new(),
                        expecting: self.determine_where_expectation(after_where),
                    };
                }
            }
        }

        // Check for FROM clause context
        if text_lower.contains("from") && !text_lower.contains("where") {
            if let Some(from_pos) = text_lower.rfind("from") {
                let after_from = &text_up_to_cursor[from_pos + 4..].trim_start();

                if after_from.is_empty() {
                    // Right after FROM keyword
                    return ParseState::InFromClause {
                        tables: Vec::new(),
                        expecting: FromExpectation::TableName,
                    };
                } else if !after_from.split_whitespace().any(|word| {
                    word.to_lowercase() == "where"
                        || word.to_lowercase() == "order"
                        || word.to_lowercase() == "group"
                }) {
                    // Still in FROM clause
                    return ParseState::InFromClause {
                        tables: Vec::new(),
                        expecting: FromExpectation::SubsequentClause,
                    };
                }
            }
        }

        // Check for SELECT clause context
        if text_lower.starts_with("select") && !text_lower.contains("from") {
            return ParseState::InSelectClause {
                items: Vec::new(),
                expecting: if text_lower.trim() == "select" {
                    SelectExpectation::ColumnOrExpression
                } else {
                    SelectExpectation::CommaOrFrom
                },
            };
        }

        ParseState::Initial
    }

    /// Determine what we're expecting in a WHERE clause based on the text
    fn determine_where_expectation(&self, after_where: &str) -> WhereExpectation {
        let tokens: Vec<&str> = after_where.split_whitespace().collect();

        if tokens.is_empty() {
            WhereExpectation::ColumnReference
        } else if tokens.len() == 1 {
            // Have a column, expecting operator
            WhereExpectation::Operator
        } else if tokens.len() == 2 {
            // Have column and operator, expecting value
            WhereExpectation::Value
        } else {
            // Have a complete condition, expecting AND/OR
            WhereExpectation::LogicalOperator
        }
    }

    /// Find the clause node that contains the cursor
    fn find_containing_clause<'a>(&self, tree: &'a Tree, cursor_point: Point) -> Option<Node<'a>> {
        let mut cursor = tree.walk();

        // Walk the tree to find clause nodes
        self.find_clause_recursive(&mut cursor, cursor_point)
    }

    /// Recursively search for clause nodes
    fn find_clause_recursive<'a>(
        &self,
        cursor: &mut tree_sitter::TreeCursor<'a>,
        point: Point,
    ) -> Option<Node<'a>> {
        let node = cursor.node();

        // Check if this is a clause node and contains the point
        if self.is_clause_node(&node) && self.point_in_node(&node, point) {
            return Some(node);
        }

        // Check children
        if cursor.goto_first_child() {
            loop {
                if let Some(found) = self.find_clause_recursive(cursor, point) {
                    cursor.goto_parent();
                    return Some(found);
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }

        None
    }

    /// Check if a node represents a SQL clause
    fn is_clause_node(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "select" | "from" | "where" | "order_by" | "group_by" | "having" | "statement" // Also consider the statement node
        )
    }

    /// Analyze statement node to determine the specific clause context
    fn analyze_statement_node(&self, node: &Node, cursor_point: Point) -> ParseState {
        // Walk through the statement's children to find what we're in
        let mut cursor = node.walk();

        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if self.point_in_node(&child, cursor_point) {
                    match child.kind() {
                        "select" => return self.analyze_select_state(&child, cursor_point),
                        "from" => return self.analyze_from_state(&child, cursor_point),
                        "where" => return self.analyze_where_state(&child, cursor_point),
                        "order_by" => return self.analyze_order_by_state(&child, cursor_point),
                        _ => {
                            // Continue checking children
                            if let Some(state) = self.analyze_nested_clause(&child, cursor_point) {
                                return state;
                            }
                        }
                    }
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        // Default to SELECT state if we're in a statement but can't determine specific clause
        ParseState::InSelectClause {
            items: Vec::new(),
            expecting: SelectExpectation::ColumnOrExpression,
        }
    }

    /// Analyze nested clauses recursively
    fn analyze_nested_clause(&self, node: &Node, cursor_point: Point) -> Option<ParseState> {
        let mut cursor = node.walk();

        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if self.point_in_node(&child, cursor_point) {
                    match child.kind() {
                        "select" => return Some(self.analyze_select_state(&child, cursor_point)),
                        "from" => return Some(self.analyze_from_state(&child, cursor_point)),
                        "where" => return Some(self.analyze_where_state(&child, cursor_point)),
                        "order_by" => {
                            return Some(self.analyze_order_by_state(&child, cursor_point));
                        }
                        _ => {
                            // Recurse deeper
                            if let Some(state) = self.analyze_nested_clause(&child, cursor_point) {
                                return Some(state);
                            }
                        }
                    }
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        None
    }

    /// Analyze SELECT clause state
    fn analyze_select_state(&self, node: &Node, _cursor_point: Point) -> ParseState {
        // Check if we have a FROM clause yet
        let has_from = self.node_has_child_of_kind(node.parent().unwrap_or(*node), "from");

        if has_from {
            // We have FROM, so we might be expecting WHERE or other clauses
            ParseState::InSelectClause {
                items: Vec::new(),
                expecting: SelectExpectation::CommaOrFrom,
            }
        } else {
            // No FROM yet, expecting columns or FROM
            ParseState::InSelectClause {
                items: Vec::new(),
                expecting: SelectExpectation::ColumnOrExpression,
            }
        }
    }

    /// Analyze FROM clause state
    fn analyze_from_state(&self, _node: &Node, _cursor_point: Point) -> ParseState {
        ParseState::InFromClause {
            tables: Vec::new(),
            expecting: FromExpectation::TableName,
        }
    }

    /// Analyze WHERE clause state
    fn analyze_where_state(&self, _node: &Node, _cursor_point: Point) -> ParseState {
        ParseState::InWhereClause {
            conditions: Vec::new(),
            expecting: WhereExpectation::ColumnReference,
        }
    }

    /// Analyze ORDER BY clause state
    fn analyze_order_by_state(&self, _node: &Node, _cursor_point: Point) -> ParseState {
        ParseState::InOrderByClause {
            items: Vec::new(),
            expecting: OrderByExpectation::ColumnName,
        }
    }

    /// Check if text looks like a keyword being typed
    fn looks_like_keyword(&self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }

        // Check if any keyword starts with this text
        self.schema
            .keywords
            .iter()
            .any(|keyword| keyword.to_lowercase().starts_with(&text.to_lowercase()))
    }

    /// Get keyword candidates for partial text
    fn get_keyword_candidates(&self, partial: &str) -> Vec<String> {
        self.schema
            .keywords
            .iter()
            .filter(|keyword| keyword.to_lowercase().starts_with(&partial.to_lowercase()))
            .cloned()
            .collect()
    }

    /// Update query scope with tables and columns from the parse tree
    fn update_query_scope(&mut self, tree: &Tree) {
        self.scope.available_tables.clear();
        self.scope.available_columns.clear();
        self.scope.aliases.clear();

        // Extract table references from the parse tree
        self.extract_table_references(tree);

        // Build available columns based on tables in scope
        self.build_available_columns();
    }

    /// Check if a string is a SQL keyword (to avoid capturing keywords as table names)
    fn is_sql_keyword(&self, word: &str) -> bool {
        let keywords = [
            "SELECT", "FROM", "WHERE", "INSERT", "UPDATE", "DELETE", "JOIN", "INNER", "LEFT",
            "RIGHT", "ON", "AND", "OR", "NOT", "IN", "EXISTS", "NULL", "TRUE", "FALSE", "LIKE",
            "BETWEEN", "ORDER", "GROUP", "BY", "HAVING", "LIMIT", "OFFSET", "UNION", "ALL",
            "DISTINCT", "AS", "CASE", "WHEN", "THEN", "ELSE", "END", "IF", "COUNT", "SUM", "AVG",
            "MIN", "MAX",
        ];
        keywords.contains(&word.to_uppercase().as_str())
    }

    /// Extract table references from the parse tree
    fn extract_table_references(&mut self, tree: &Tree) {
        let query = r#"(relation (object_reference (identifier) @table.name))"#;
        let query = Query::new(&tree_sitter_sequel::LANGUAGE.into(), query).unwrap();
        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(&query, tree.root_node(), self.query.as_bytes());

        while let Some(query_match) = matches.next() {
            for capture in query_match.captures {
                let capture_name = query.capture_names()[capture.index as usize];
                if capture_name == "table.name" {
                    let table_name = capture.node.utf8_text(self.query.as_bytes()).unwrap();

                    let cleaned_name = table_name.trim_matches('"').trim();
                    if !cleaned_name.is_empty() && !self.is_sql_keyword(cleaned_name) {
                        self.scope
                            .available_tables
                            .insert(cleaned_name.to_string(), cleaned_name.to_string());
                    }
                }
            }
        }
    }

    /// Extract the next identifier from text (handles quoted identifiers)
    fn extract_next_identifier(&self, text: &str) -> Option<String> {
        let trimmed = text.trim_start();

        if trimmed.starts_with('"') {
            // Quoted identifier
            if let Some(end_quote) = trimmed[1..].find('"') {
                return Some(trimmed[1..end_quote + 1].to_string());
            }
        } else {
            // Unquoted identifier
            let identifier = trimmed
                .split_whitespace()
                .next()?
                .chars()
                .take_while(|&c| c.is_alphanumeric() || c == '_')
                .collect::<String>();

            if !identifier.is_empty() {
                return Some(identifier);
            }
        }

        None
    }

    /// Build available columns based on tables in scope
    fn build_available_columns(&mut self) {
        for table_name in self.scope.available_tables.keys() {
            if let Some(table_info) = self.schema.tables.get(table_name) {
                for column in &table_info.columns {
                    self.scope.available_columns.push(ScopedColumn {
                        table_name: table_name.clone(),
                        table_alias: None, // TODO: Handle aliases
                        column_name: column.name.clone(),
                        data_type: column.data_type.clone(),
                        fully_qualified_name: format!("{}.{}", table_name, column.name),
                    });
                }
            }
        }
    }

    /// Get the last word before the cursor at a given position
    fn get_last_word_at_position(&self, position: Point) -> Option<String> {
        let cursor_byte = self.point_to_byte(position);
        let text_up_to_cursor = &self.query[..cursor_byte.min(self.query.len())];

        // Find the last word boundary
        let mut word_start = None;
        for (i, ch) in text_up_to_cursor.char_indices().rev() {
            if ch.is_whitespace() || ch == '(' || ch == ')' || ch == ',' {
                word_start = Some(i + ch.len_utf8());
                break;
            }
        }

        let start = word_start.unwrap_or(0);
        if start < text_up_to_cursor.len() {
            Some(text_up_to_cursor[start..].to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_parsing_suggestion_types() {
        let mut context = ParsingContext::new().expect("Failed to initialize parser");

        let test_queries = vec![
            ("SELECT ", Point { row: 0, column: 7 }),
            ("SELECT * ", Point { row: 0, column: 9 }),
            ("SELECT * FROM ", Point { row: 0, column: 14 }),
            ("SELECT * FROM users ", Point { row: 0, column: 20 }),
            ("SELECT * FROM users WHERE ", Point { row: 0, column: 26 }),
            (
                "SELECT * FROM users WHERE id ",
                Point { row: 0, column: 29 },
            ),
        ];

        // Expected suggestion types for each query
        let expected_types = vec![
            SuggestionType::Mixed(vec![
                SuggestionCategory::Columns { tables: vec![] },
                SuggestionCategory::Functions,
                SuggestionCategory::Keywords,
            ]),
            SuggestionType::Keywords,
            SuggestionType::Tables,
            SuggestionType::Keywords,
            SuggestionType::Columns { tables: vec![] },
            SuggestionType::Keywords,
        ];

        for ((query, cursor_pos), expected) in
            test_queries.into_iter().zip(expected_types.into_iter())
        {
            // Simulate LSP didChange event
            context
                .did_change(query.to_string())
                .expect(&format!("Failed to parse query: '{}'", query));

            // Simulate LSP completion request
            let actual = context
                .suggestion_for_position(cursor_pos)
                .expect(&format!("Failed to get completions for query: '{}'", query));

            // Assert the suggestion type matches expected
            assert_eq!(
                std::mem::discriminant(&actual),
                std::mem::discriminant(&expected),
                "Query '{}' at position {:?} should suggest {:?}, but got {:?}",
                query,
                cursor_pos,
                expected,
                actual
            );

            // For more specific assertions, check the contents
            match (&actual, &expected) {
                (SuggestionType::Mixed(actual_cats), SuggestionType::Mixed(expected_cats)) => {
                    assert_eq!(
                        actual_cats.len(),
                        expected_cats.len(),
                        "Query '{}': Mixed suggestion categories count mismatch",
                        query
                    );

                    for (actual_cat, expected_cat) in actual_cats.iter().zip(expected_cats.iter()) {
                        assert_eq!(
                            std::mem::discriminant(actual_cat),
                            std::mem::discriminant(expected_cat),
                            "Query '{}': Mixed suggestion category mismatch",
                            query
                        );
                    }
                }
                (
                    SuggestionType::Columns {
                        tables: actual_tables,
                    },
                    SuggestionType::Columns {
                        tables: expected_tables,
                    },
                ) => {
                    assert_eq!(
                        actual_tables, expected_tables,
                        "Query '{}': Columns tables mismatch",
                        query
                    );
                }
                _ => {
                    // For other types, the discriminant check above is sufficient
                }
            }
        }
    }

    fn get_suggestion_type_name(suggestion_type: &SuggestionType) -> &'static str {
        match suggestion_type {
            SuggestionType::Keywords => "Keywords",
            SuggestionType::Tables => "Tables",
            SuggestionType::Columns { .. } => "Columns",
            SuggestionType::Functions => "Functions",
            SuggestionType::Values(_) => "Values",
            SuggestionType::Mixed(_) => "Mixed",
        }
    }
}
