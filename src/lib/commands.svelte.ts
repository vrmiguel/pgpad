import { invoke } from '@tauri-apps/api/core';

// What Rust sends us after processing query results (basically, JSON)
export type Json = string | number | boolean | null | Json[] | { [key: string]: Json };

export type Row = Json[];

export type QueryId = number;
export type QueryStatus = 'Pending' | 'Running' | 'Completed' | 'Error';
export type Page = Json[][];

export interface StatementInfo {
	returns_values: boolean;
	status: QueryStatus;
	first_page: Page | null;
	affected_rows: number | null;
	error: string | null;
}

export type DatabaseInfo =
	| { Postgres: { connection_string: string } }
	| { SQLite: { db_path: string } };

export interface ConnectionInfo {
	id: string;
	name: string;
	connected: boolean;
	database_type: DatabaseInfo;
}

export interface QueryHistoryEntry {
	id: number;
	connection_id: string;
	query_text: string;
	executed_at: number;
	duration_ms: number | null;
	status: string;
	row_count: number;
	error_message: string | null;
}

export interface ColumnInfo {
	name: string;
	data_type: string;
	is_nullable: boolean;
	default_value: string | null;
}

export interface TableInfo {
	name: string;
	schema: string;
	columns: ColumnInfo[];
}

export interface DatabaseSchema {
	tables: TableInfo[];
	schemas: string[];
	unique_columns: string[];
}

export interface Script {
	id: number;
	name: string;
	description: string | null;
	query_text: string;
	connection_id: string | null;
	tags: string | null;
	created_at: number;
	updated_at: number;
	favorite: boolean;
}

export class Commands {
	static async testConnection(databaseInfo: DatabaseInfo): Promise<boolean> {
		return await invoke('test_connection', { databaseInfo });
	}

	static async addConnection(name: string, databaseInfo: DatabaseInfo): Promise<ConnectionInfo> {
		return await invoke('add_connection', { name, databaseInfo });
	}

	static async connectToDatabase(connectionId: string): Promise<boolean> {
		return await invoke('connect_to_database', { connectionId });
	}

	static async disconnectFromDatabase(connectionId: string): Promise<void> {
		return await invoke('disconnect_from_database', { connectionId });
	}

	static async getConnections(): Promise<ConnectionInfo[]> {
		return await invoke('get_connections');
	}

	static async removeConnection(connectionId: string): Promise<void> {
		return await invoke('remove_connection', { connectionId });
	}

	static async updateConnection(
		connectionId: string,
		name: string,
		databaseInfo: DatabaseInfo
	): Promise<ConnectionInfo> {
		return await invoke('update_connection', { connId: connectionId, name, databaseInfo });
	}

	static async initializeConnections(): Promise<void> {
		return await invoke('initialize_connections');
	}

	static async saveQueryToHistory(
		connectionId: string,
		query: string,
		durationMs?: number,
		status: string = 'success',
		rowCount: number = 0,
		errorMessage?: string
	): Promise<void> {
		await invoke('save_query_to_history', {
			connectionId,
			query,
			durationMs,
			status,
			rowCount,
			errorMessage
		});
	}

	static async getQueryHistory(connectionId: string, limit?: number): Promise<QueryHistoryEntry[]> {
		return await invoke('get_query_history', { connectionId, limit });
	}

	static async getDatabaseSchema(connectionId: string): Promise<DatabaseSchema> {
		return await invoke('get_database_schema', { connectionId });
	}

	static async saveScript(
		name: string,
		content: string,
		connectionId?: string,
		description?: string
	): Promise<number> {
		return await invoke('save_script', {
			name,
			content,
			connectionId: connectionId || null,
			description: description || null
		});
	}

	static async updateScript(
		id: number,
		name: string,
		content: string,
		connectionId?: string,
		description?: string
	): Promise<void> {
		return await invoke('update_script', {
			id,
			name,
			content,
			connectionId: connectionId || null,
			description: description || null
		});
	}

	static async getScripts(connectionId?: string): Promise<Script[]> {
		return await invoke('get_scripts', { connectionId: connectionId || null });
	}

	static async deleteScript(id: number): Promise<void> {
		await invoke('delete_script', { id });
	}

	static async minimizeWindow(): Promise<void> {
		await invoke('minimize_window');
	}

	static async maximizeWindow(): Promise<void> {
		await invoke('maximize_window');
	}

	static async closeWindow(): Promise<void> {
		await invoke('close_window');
	}

	static async saveSessionState(sessionData: string): Promise<void> {
		return await invoke('save_session_state', { sessionData });
	}

	static async getSessionState(): Promise<string | null> {
		return await invoke('get_session_state');
	}

	static async pickSqliteDbDialog(): Promise<string | null> {
		return await invoke('open_sqlite_db');
	}

	static async saveSqliteDbDialog(): Promise<string | null> {
		return await invoke('save_sqlite_db');
	}

	static async startQuery(connectionId: string, query: string): Promise<QueryId[]> {
		return await invoke('start_query', { connectionId, query });
	}

	static async fetchQuery(queryId: QueryId): Promise<StatementInfo> {
		return await invoke('fetch_query', { queryId });
	}

	static async fetchPage(queryId: QueryId, pageIndex: number): Promise<Page | null> {
		return await invoke('fetch_page', { queryId, pageIndex });
	}

	static async getQueryStatus(queryId: QueryId): Promise<QueryStatus> {
		return await invoke('get_query_status', { queryId });
	}

	static async getPageCount(queryId: QueryId): Promise<number> {
		return await invoke('get_page_count', { queryId });
	}

	static async getColumns(queryId: QueryId): Promise<string[] | null> {
		return await invoke('get_columns', { queryId });
	}
}
