import {
	Commands,
	type QueryId,
	type Page,
	type QueryStatus,
	type QuerySnapshot
} from '$lib/commands.svelte';
import { SvelteMap } from 'svelte/reactivity';

export interface QueryResultTab {
	id: number;
	queryId: QueryId;
	name: string;
	query: string;
	timestamp: number;
	status: QueryStatus;
	queryReturnsResults?: boolean;
	affectedRows?: number;
	columns?: string[];
	currentPageIndex: number;
	currentPageData: Page | null;
	totalPages: number | null;
	error?: string;
}

export class QueryExecutor {
	resultTabs = $state<QueryResultTab[]>([]);
	activeResultTabId = $state<number | null>(null);

	private nextResultTabId = 1;
	private latestPageRequests = new SvelteMap<QueryId, number>();
	private onComplete?: (totalRows: number) => void;
	private executionId = 0;
	private pollingAbortController: AbortController | null = null;

	constructor() {}

	dispose() {
		this.stopPollingLoop();
		this.executionId++;
		this.latestPageRequests.clear();
	}

	async executeQuery(
		queryText: string,
		connectionId: string,
		onComplete?: (totalRows: number) => void
	) {
		const currentExecutionId = ++this.executionId;
		// Store callback for use in completion handlers
		this.onComplete = onComplete;
		// Clear previous results
		this.stopPollingLoop();
		this.latestPageRequests.clear();
		this.resultTabs = [];
		this.activeResultTabId = null;
		this.nextResultTabId = 1;

		try {
			const queryIds = await Commands.submitQuery(connectionId, queryText.trim());

			// Create a tab per statement
			for (const queryId of queryIds) {
				await this.initializeQueryTab(queryId, queryText, currentExecutionId);
			}
		} catch (error) {
			if (currentExecutionId !== this.executionId) {
				return;
			}
			console.error('Failed to execute query:', error);

			const errorMsg = error instanceof Error ? error.message : String(error);
			const tabId = this.nextResultTabId++;

			const errorTab: QueryResultTab = {
				id: tabId,
				queryId: -1,
				name: this.generateTabTitle(queryText),
				query: queryText,
				timestamp: Date.now(),
				status: 'Error',
				currentPageIndex: 0,
				currentPageData: null,
				totalPages: null,
				error: errorMsg
			};

			this.resultTabs = [errorTab];
			this.activeResultTabId = tabId;
		}
	}

	private async initializeQueryTab(queryId: QueryId, queryText: string, executionId: number) {
		if (executionId !== this.executionId) return;

		const tabId = this.nextResultTabId++;

		const newTab: QueryResultTab = {
			id: tabId,
			queryId,
			name: `Query ${queryId}`,
			query: queryText,
			timestamp: Date.now(),
			status: 'Running',
			currentPageIndex: 0,
			currentPageData: null,
			totalPages: null
		};

		this.resultTabs = [...this.resultTabs, newTab];
		this.activeResultTabId = tabId;

		const info = await this.waitUntilRenderable(queryId);
		if (executionId !== this.executionId) return;

		const tabIndex = this.resultTabs.findIndex((t) => t.id === tabId);
		if (tabIndex < 0) return;

		if (info.error) {
			this.resultTabs[tabIndex] = {
				...this.resultTabs[tabIndex],
				status: 'Error',
				error: info.error
			};
			this.resultTabs = [...this.resultTabs];
			return;
		}

		const tabName = this.generateTabTitle(queryText);

		if (!info.returns_values) {
			this.resultTabs[tabIndex] = {
				...this.resultTabs[tabIndex],
				status: info.status,
				name: tabName,
				queryReturnsResults: false,
				affectedRows: info.affected_rows ?? undefined
			};
			this.resultTabs = [...this.resultTabs];

			if (info.status === 'Completed') {
				this.onComplete?.(info.affected_rows ?? 0);
			}

			return;
		}

		if (!info.columns) {
			this.resultTabs[tabIndex] = {
				...this.resultTabs[tabIndex],
				status: 'Error',
				error: 'Failed to get column information'
			};
			this.resultTabs = [...this.resultTabs];
			return;
		}

		this.resultTabs[tabIndex] = {
			...this.resultTabs[tabIndex],
			columns: info.columns,
			name: tabName,
			currentPageData: info.first_page,
			status: info.status,
			queryReturnsResults: true
		};
		this.resultTabs = [...this.resultTabs];

		if (info.status === 'Completed') {
			const pageCount = await Commands.getPageCount(queryId);
			const tabIdx = this.resultTabs.findIndex((t) => t.id === tabId);
			if (tabIdx >= 0) {
				this.resultTabs[tabIdx] = { ...this.resultTabs[tabIdx], totalPages: pageCount };
				this.resultTabs = [...this.resultTabs];
			}

			this.onComplete?.((pageCount || 0) * 50);
		} else {
			this.startPollingLoop(executionId);
		}
	}

	private async waitUntilRenderable(queryId: QueryId): Promise<QuerySnapshot> {
		const now = performance.now();
		const res = await Commands.waitUntilRenderable(queryId);
		const elapsed = performance.now() - now;
		console.log('Wait until renderable took', elapsed, 'ms');
		return res;
	}

	private async pollForPage(queryId: QueryId, pageIndex: number): Promise<Page | null> {
		for (let i = 0; i < 100; i++) {
			const page = await Commands.fetchPage(queryId, pageIndex);
			if (page) {
				return page;
			}
			await new Promise((resolve) => setTimeout(resolve, 100));
		}
		console.error('Timeout waiting for page', pageIndex, 'for queryId:', queryId);
		return null;
	}

	private startPollingLoop(executionId: number) {
		if (this.pollingAbortController && !this.pollingAbortController.signal.aborted) {
			return;
		}

		const controller = new AbortController();
		this.pollingAbortController = controller;

		void this.runPollingLoop(executionId, controller.signal);
	}

	private stopPollingLoop() {
		this.pollingAbortController?.abort();
		this.pollingAbortController = null;
	}

	private async runPollingLoop(executionId: number, signal: AbortSignal) {
		while (!signal.aborted && executionId === this.executionId) {
			const runningQueryIds = this.resultTabs
				.filter((t) => t.status === 'Running' && t.queryReturnsResults !== false)
				.map((t) => t.queryId);

			if (runningQueryIds.length === 0) {
				break;
			}

			for (const queryId of runningQueryIds) {
				if (signal.aborted || executionId !== this.executionId) {
					return;
				}
				await this.pollQueryProgress(queryId);
			}

			await this.sleep(200, signal);
		}

		if (this.pollingAbortController?.signal === signal) {
			this.pollingAbortController = null;
		}
	}

	private async pollQueryProgress(queryId: QueryId) {
		try {
			const status = await Commands.getQueryStatus(queryId);
			const pageCount = await Commands.getPageCount(queryId);

			const tabIndex = this.resultTabs.findIndex((t) => t.queryId === queryId);
			if (tabIndex < 0) return;
			const tab = this.resultTabs[tabIndex];

			let changed = false;
			if (tab.totalPages !== pageCount) {
				this.resultTabs[tabIndex] = { ...this.resultTabs[tabIndex], totalPages: pageCount };
				changed = true;
			}

			if (tab.status !== status) {
				this.resultTabs[tabIndex] = { ...this.resultTabs[tabIndex], status };
				changed = true;
			}

			if (changed) {
				this.resultTabs = [...this.resultTabs];
			}

			if (status === 'Completed') {
				const totalRows = (pageCount || 0) * 50;
				this.onComplete?.(totalRows);
			}
		} catch (error) {
			console.error('Error polling for page count:', error);
			this.stopPollingLoop();
		}
	}

	private async sleep(ms: number, signal?: AbortSignal): Promise<void> {
		await new Promise<void>((resolve) => {
			if (signal?.aborted) {
				resolve();
				return;
			}

			const timeout = setTimeout(() => {
				signal?.removeEventListener('abort', onAbort);
				resolve();
			}, ms);

			const onAbort = () => {
				clearTimeout(timeout);
				resolve();
			};

			signal?.addEventListener('abort', onAbort, { once: true });
		});
	}

	async loadPage(queryId: QueryId, pageIndex: number) {
		const tabIndex = this.resultTabs.findIndex((t) => t.queryId === queryId);
		if (tabIndex < 0) return;

		const requestId = (this.latestPageRequests.get(queryId) ?? 0) + 1;
		this.latestPageRequests.set(queryId, requestId);

		const page = await this.pollForPage(queryId, pageIndex);
		if (this.latestPageRequests.get(queryId) !== requestId) return;
		if (page) {
			this.resultTabs[tabIndex] = {
				...this.resultTabs[tabIndex],
				currentPageIndex: pageIndex,
				currentPageData: page
			};
			this.resultTabs = [...this.resultTabs];
		}
	}

	private generateTabTitle(query: string): string {
		const cleaned = query.trim().replace(/\s+/g, ' ');
		if (cleaned.length <= 30) return cleaned;
		return cleaned.substring(0, 27) + '...';
	}

	// Note: for these functions that get passed as callbacks, do use arrow functions to preserve the binding of `this`
	handleResultTabClose = (tabId: number) => {
		this.resultTabs = this.resultTabs.filter((tab) => tab.id !== tabId);

		if (this.activeResultTabId === tabId) {
			this.activeResultTabId = this.resultTabs.length > 0 ? this.resultTabs[0].id : null;
		}
	};

	handleResultTabSelect = (tabId: number) => {
		this.activeResultTabId = tabId;
	};

	getTabStatus = (tab: QueryResultTab): 'normal' | 'modified' | 'error' => {
		switch (tab.status) {
			case 'Error':
				return 'error';
			case 'Running':
				return 'modified';
			default:
				return 'normal';
		}
	};
}
