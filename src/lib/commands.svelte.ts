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
}
