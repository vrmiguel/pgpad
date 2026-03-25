CREATE TABLE database_types (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE NOT NULL
);

INSERT INTO database_types (id, name) VALUES (1, 'postgres'), (2, 'sqlite');

CREATE TABLE connections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    connection_data TEXT NOT NULL,
    database_type_id INTEGER NOT NULL DEFAULT 1 REFERENCES database_types(id),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_connected_at INTEGER,
    favorite BOOLEAN DEFAULT FALSE,
    color TEXT,
    sort_order INTEGER DEFAULT 0
);

CREATE TABLE query_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    connection_id TEXT NOT NULL,
    query_text TEXT NOT NULL,
    executed_at INTEGER NOT NULL,
    duration_ms INTEGER,
    status TEXT NOT NULL,
    row_count INTEGER DEFAULT 0,
    error_message TEXT,
    FOREIGN KEY (connection_id) REFERENCES connections(id) ON DELETE CASCADE
);

CREATE TABLE saved_queries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    query_text TEXT NOT NULL,
    connection_id TEXT,
    tags TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    favorite BOOLEAN DEFAULT FALSE,
    FOREIGN KEY (connection_id) REFERENCES connections(id) ON DELETE SET NULL
);

CREATE TABLE connection_groups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    color TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE connection_group_members (
    connection_id TEXT NOT NULL,
    group_id INTEGER NOT NULL,
    PRIMARY KEY (connection_id, group_id),
    FOREIGN KEY (connection_id) REFERENCES connections(id) ON DELETE CASCADE,
    FOREIGN KEY (group_id) REFERENCES connection_groups(id) ON DELETE CASCADE
);

CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE schema_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    connection_id TEXT NOT NULL,
    schema_name TEXT NOT NULL,
    table_name TEXT NOT NULL,
    column_name TEXT,
    data_type TEXT,
    cached_at INTEGER NOT NULL,
    FOREIGN KEY (connection_id) REFERENCES connections(id) ON DELETE CASCADE
);

CREATE INDEX idx_query_history_connection_id ON query_history(connection_id);
CREATE INDEX idx_query_history_executed_at ON query_history(executed_at DESC);
CREATE INDEX idx_query_history_status ON query_history(status);

CREATE INDEX idx_saved_queries_connection_id ON saved_queries(connection_id);
CREATE INDEX idx_saved_queries_favorite ON saved_queries(favorite);
CREATE INDEX idx_saved_queries_created_at ON saved_queries(created_at DESC);

CREATE INDEX idx_connections_favorite ON connections(favorite);
CREATE INDEX idx_connections_sort_order ON connections(sort_order);
CREATE INDEX idx_connections_last_connected ON connections(last_connected_at DESC);

CREATE INDEX idx_schema_cache_connection ON schema_cache(connection_id, schema_name, table_name);