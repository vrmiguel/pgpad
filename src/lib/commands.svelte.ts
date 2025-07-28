import { invoke } from '@tauri-apps/api/core';

export const preventDefault = <T extends Event>(fn: (e: T) => void): ((e: T) => void) => {
	return (e: T) => {
		e.preventDefault();
		fn(e);
	};
};

export enum FILES {
	GREET_FILE = 'greet.txt',
	NAME_FILE = 'name.txt'
}

export class GlobalState {
	private _state = $state({ name: '', greet: '' });

	get greet() {
		return this._state.greet;
	}

	set greet(value: string) {
		this._state.greet = value;
	}

	get name() {
		return this._state.name;
	}

	set name(value: string) {
		this._state.name = value;
	}

	get nlen() {
		return this.name.length;
	}

	get glen() {
		return this.greet.length;
	}

	async read(path: FILES) {
		const contentFromFile = await invoke<string>('read', { path });
		if (path === FILES.NAME_FILE) {
			this.name = contentFromFile;
		} else if (path === FILES.GREET_FILE) {
			this.greet = contentFromFile;
		}
	}

	async write(path: FILES, contents: string) {
		await invoke('write', { path, contents });
		if (path === FILES.NAME_FILE) {
			this.name = contents;
		} else if (path === FILES.GREET_FILE) {
			this.greet = contents;
		}
	}

	reset() {
		this.name = '';
		this.greet = '';
	}
}

export interface ConnectionConfig {
	name: string;
	connection_string: string;
}

export interface ConnectionInfo {
	id: string;
	name: string;
	connection_string: string;
	connected: boolean;
}

export interface QueryResult {
	columns: string[];
	rows: any[][];
	row_count: number;
	duration_ms: number;
}

// UI-facing result type that includes success/error state
export interface QueryResultUI {
	success: boolean;
	data?: any[];
	columns?: string[];
	message?: string;
	duration?: number;
	queryResult?: QueryResult;
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

export class DatabaseCommands {
	static async testConnection(config: ConnectionConfig): Promise<boolean> {
		return await invoke('test_connection', { config });
	}

	static async addConnection(config: ConnectionConfig): Promise<ConnectionInfo> {
		return await invoke('add_connection', { config });
	}

	static async connectToDatabase(connectionId: string): Promise<boolean> {
		return await invoke('connect_to_database', { connectionId });
	}

	static async disconnectFromDatabase(connectionId: string): Promise<void> {
		return await invoke('disconnect_from_database', { connectionId });
	}

	static async executeQuery(connectionId: string, query: string): Promise<QueryResult> {
		return await invoke('execute_query', { connectionId, query });
	}

	static async getConnections(): Promise<ConnectionInfo[]> {
		return await invoke('get_connections');
	}

	static async removeConnection(connectionId: string): Promise<void> {
		return await invoke('remove_connection', { connectionId });
	}

	static async initializeConnections(): Promise<void> {
		return await invoke('initialize_connections');
	}

	static async saveQueryToHistory(
		connectionId: string,
		query: string,
		durationMs: number | null,
		status: string,
		rowCount: number,
		errorMessage: string | null
	): Promise<void> {
		return await invoke('save_query_to_history', {
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
		return await invoke('delete_script', { id });
	}
}
