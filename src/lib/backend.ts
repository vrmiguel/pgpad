import { invoke as tauriInvoke, isTauri } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';
import { getWebToken } from '$lib/webToken';

type CommandArgs = Record<string, unknown>;
type Unlisten = () => void;

interface CommandError {
	message?: string;
}

export interface Backend {
	invoke<T>(command: string, args?: CommandArgs): Promise<T>;
	listen<T>(event: string, handler: (payload: T) => void): Promise<Unlisten>;
}

class TauriBackend implements Backend {
	async invoke<T>(command: string, args?: CommandArgs): Promise<T> {
		return await tauriInvoke<T>(command, args);
	}

	async listen<T>(event: string, handler: (payload: T) => void): Promise<Unlisten> {
		return await tauriListen<T>(event, (event) => handler(event.payload));
	}
}

class HttpBackend implements Backend {
	async invoke<T>(command: string, args?: CommandArgs): Promise<T> {
		const response = await fetch(`/api/commands/${encodeURIComponent(command)}`, {
			method: 'POST',
			headers: this.commandHeaders(),
			body: JSON.stringify(args ?? {})
		});

		if (!response.ok) {
			throw new Error(await this.readError(response));
		}

		if (response.status === 204) {
			return undefined as T;
		}

		const text = await response.text();
		return text ? (JSON.parse(text) as T) : (undefined as T);
	}

	async listen<T>(event: string, handler: (payload: T) => void): Promise<Unlisten> {
		void event;
		void handler;
		return () => {};
	}

	private commandHeaders(): HeadersInit {
		const headers: Record<string, string> = {
			'Content-Type': 'application/json'
		};
		const token = getWebToken();
		if (token) {
			headers['x-pgpad-token'] = token;
		}

		return headers;
	}

	private async readError(response: Response) {
		const text = await response.text();
		if (!text) {
			return `Command failed with HTTP ${response.status}`;
		}

		try {
			const error = JSON.parse(text) as CommandError;
			return error.message ?? text;
		} catch {
			return text;
		}
	}
}

export const backend: Backend = isTauri() ? new TauriBackend() : new HttpBackend();
