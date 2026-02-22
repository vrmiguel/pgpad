import type { Transport } from '@codemirror/lsp-client';
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';

export class TauriLSPTransport implements Transport {
	private handlers: Array<(message: string) => void> = [];
	private unlisten: UnlistenFn | null = null;

	constructor() {
		void this.initializeListener();
	}

	private async initializeListener(): Promise<void> {
		this.unlisten = await listen<string>('lsp-response', (event) => {
			this.handlers.forEach((handler) => handler(event.payload));
		});
	}

	send(message: string): void {
		void emit('lsp-request', message);
	}

	subscribe(handler: (message: string) => void): void {
		this.handlers.push(handler);
	}

	unsubscribe(handler: (message: string) => void): void {
		this.handlers = this.handlers.filter((current) => current !== handler);
	}

	dispose(): void {
		this.unlisten?.();
		this.unlisten = null;
		this.handlers = [];
	}

	async updateSelectedConnection(connectionId: string | null): Promise<void> {
		const message = JSON.stringify({
			jsonrpc: '2.0',
			id: Date.now(),
			method: 'pgpad/connectionSelected',
			params: connectionId
		});

		await emit('lsp-request', message);
	}
}
