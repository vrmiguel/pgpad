import {
	Commands,
	type QueryId,
	type Page,
	type QueryStatus,
	type StatementInfo
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
	private pageCountPolls = new SvelteMap<QueryId, ReturnType<typeof setInterval>>();
	private onComplete?: (totalRows: number) => void;

	constructor() {}

	dispose() {
		for (const interval of this.pageCountPolls.values()) {
			clearInterval(interval);
		}
		this.pageCountPolls.clear();
	}

	async executeQuery(
		queryText: string,
		connectionId: string,
		onComplete?: (totalRows: number) => void
	) {
		// Store callback for use in completion handlers
		this.onComplete = onComplete;
		// Clear previous results
		for (const interval of this.pageCountPolls.values()) {
			clearInterval(interval);
		}
		this.pageCountPolls.clear();
		this.resultTabs = [];
		this.activeResultTabId = null;
		this.nextResultTabId = 1;

		try {
			const queryIds = await Commands.submitQuery(connectionId, queryText.trim());

			// Create a tab per statement
			for (const queryId of queryIds) {
				await this.initializeQueryTab(queryId, queryText);
			}
		} catch (error) {
			console.error('Failed to execute query:', error);

			const errorObj = error as { message: string };
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
				error: errorObj.message
			};

			this.resultTabs = [errorTab];
			this.activeResultTabId = tabId;
		}
	}

	private async initializeQueryTab(queryId: QueryId, queryText: string) {
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

		const columns = await this.pollForColumns(queryId);
		if (!columns) {
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
			columns,
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
			this.pollForPageCount(queryId);
		}
	}

	private async waitUntilRenderable(queryId: QueryId): Promise<StatementInfo> {
		const now = performance.now();
		const res = await Commands.waitUntilRenderable(queryId);
		const elapsed = performance.now() - now;
		console.log('Wait until renderable took', elapsed, 'ms');
		return res;
	}

	private async pollForColumns(queryId: QueryId): Promise<string[] | null> {
		for (let i = 0; i < 50; i++) {
			const columns = await Commands.getColumns(queryId);
			if (columns) {
				return columns;
			}
			await new Promise((resolve) => setTimeout(resolve, 100));
		}
		return null;
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

	private pollForPageCount(queryId: QueryId) {
		const poll = async () => {
			try {
				const status = await Commands.getQueryStatus(queryId);
				const pageCount = await Commands.getPageCount(queryId);

				const tab = this.resultTabs.find((t) => t.queryId === queryId);
				if (!tab) return;

				let changed = false;

				if (tab.totalPages !== pageCount) {
					const tabIndex = this.resultTabs.findIndex((t) => t.queryId === queryId);
					if (tabIndex >= 0) {
						this.resultTabs[tabIndex] = { ...this.resultTabs[tabIndex], totalPages: pageCount };
						changed = true;
					}
				}

				if (tab.status !== status) {
					const tabIndex = this.resultTabs.findIndex((t) => t.queryId === queryId);
					if (tabIndex >= 0) {
						this.resultTabs[tabIndex] = { ...this.resultTabs[tabIndex], status };
						changed = true;
					}
				}

				if (changed) {
					this.resultTabs = [...this.resultTabs];
				}

				if (status === 'Completed') {
					const interval = this.pageCountPolls.get(queryId);
					if (interval) {
						clearInterval(interval);
						this.pageCountPolls.delete(queryId);
					}

					const totalRows = (tab.totalPages || 0) * 50;
					this.onComplete?.(totalRows);
				}
			} catch (error) {
				console.error('Error polling for page count:', error);
			}
		};

		poll();
		const interval = setInterval(poll, 200);
		this.pageCountPolls.set(queryId, interval);
	}

	async loadPage(queryId: QueryId, pageIndex: number) {
		const tabIndex = this.resultTabs.findIndex((t) => t.queryId === queryId);
		if (tabIndex < 0) return;

		const page = await this.pollForPage(queryId, pageIndex);
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
