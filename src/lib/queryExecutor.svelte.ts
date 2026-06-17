import {
	Commands,
	type QueryId,
	type Page,
	type QueryStatus,
	type QueryEvent
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
	private generation = 0;
	private unlistenQueryEvents: (() => void) | null = null;
	private trackedQueries = new SvelteMap<QueryId, QueryStatus>();
	private currentQueryText = '';
	private queryEventListenerReady: Promise<void>;
	private queryEventListenerError: unknown = null;
	private disposed = false;

	constructor() {
		this.queryEventListenerReady = this.startQueryEventListener();
	}

	dispose() {
		this.disposed = true;
		this.stopQueryEventListener();
		this.generation++;
		this.latestPageRequests.clear();
		this.trackedQueries.clear();
	}

	async executeQuery(
		queryText: string,
		connectionId: string,
		onComplete?: (totalRows: number) => void
	) {
		const currentGeneration = ++this.generation;
		// Store callback for use in completion handlers
		this.onComplete = onComplete;
		this.currentQueryText = queryText;
		this.latestPageRequests.clear();
		this.trackedQueries.clear();

		try {
			await this.queryEventListenerReady;
			if (this.queryEventListenerError) {
				throw this.queryEventListenerError;
			}

			const queryIds = await Commands.submitQuery(connectionId, queryText.trim());

			if (currentGeneration !== this.generation) return;

			if (this.trackedQueries.size === 0) {
				this.createResultTabs(queryIds, queryText);
			}
		} catch (error) {
			if (currentGeneration !== this.generation) {
				return;
			}
			console.error('Failed to execute query:', error);

			this.latestPageRequests.clear();
			this.trackedQueries.clear();

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

	async loadPage(queryId: QueryId, pageIndex: number) {
		const tabIndex = this.resultTabs.findIndex((t) => t.queryId === queryId);
		if (tabIndex < 0) return;

		const requestId = (this.latestPageRequests.get(queryId) ?? 0) + 1;
		this.latestPageRequests.set(queryId, requestId);

		const page = await Commands.fetchPage(queryId, pageIndex);
		if (this.latestPageRequests.get(queryId) !== requestId) return;
		const latestTabIndex = this.resultTabs.findIndex((t) => t.queryId === queryId);
		if (latestTabIndex < 0) return;

		if (page) {
			this.resultTabs[latestTabIndex] = {
				...this.resultTabs[latestTabIndex],
				currentPageIndex: pageIndex,
				currentPageData: page
			};
			this.resultTabs = [...this.resultTabs];
		}
	}

	private async startQueryEventListener() {
		if (this.unlistenQueryEvents) return;

		try {
			const unlisten = await Commands.listenQueryEvents((event) => {
				void this.handleQueryEvent(event);
			});

			if (this.disposed) {
				unlisten();
				return;
			}

			this.unlistenQueryEvents = unlisten;
		} catch (error) {
			this.queryEventListenerError = error;
			console.error('Failed to listen for query events:', error);
		}
	}

	private stopQueryEventListener() {
		this.unlistenQueryEvents?.();
		this.unlistenQueryEvents = null;
	}

	private async handleQueryEvent(event: QueryEvent) {
		const generation = this.generation;

		if (event.type === 'submitted') {
			if (this.trackedQueries.size === 0) {
				this.createResultTabs(event.query_ids, this.currentQueryText);
			}
			return;
		}

		if (this.trackedQueries.size === 0) {
			return;
		}

		if (!this.trackedQueries.has(event.query_id)) return;

		switch (event.type) {
			case 'columns_ready':
				this.applyColumnsReady(event);
				break;
			case 'page_ready':
				await this.applyPageReady(event, generation);
				break;
			case 'finished':
				this.applyFinished(event);
				break;
		}
	}

	private createResultTabs(queryIds: QueryId[], queryText: string) {
		this.latestPageRequests.clear();
		this.resultTabs = [];
		this.activeResultTabId = null;
		this.nextResultTabId = 1;

		const newTabs: QueryResultTab[] = queryIds.map((queryId, index) => {
			const tabId = this.nextResultTabId++;
			const baseTitle = this.generateTabTitle(queryText);
			const title =
				queryIds.length > 1 ? `${baseTitle} (${index + 1}/${queryIds.length})` : baseTitle;

			return {
				id: tabId,
				queryId,
				name: title,
				query: queryText,
				timestamp: Date.now(),
				status: 'Running',
				currentPageIndex: 0,
				currentPageData: null,
				totalPages: null
			};
		});

		this.resultTabs = newTabs;
		this.activeResultTabId = newTabs[0]?.id ?? null;
		this.trackedQueries = new SvelteMap(queryIds.map((queryId) => [queryId, 'Running']));
	}

	private applyColumnsReady(event: Extract<QueryEvent, { type: 'columns_ready' }>) {
		const tabIndex = this.resultTabs.findIndex((t) => t.queryId === event.query_id);
		if (tabIndex < 0) return;

		this.resultTabs[tabIndex] = {
			...this.resultTabs[tabIndex],
			columns: event.columns,
			queryReturnsResults: true
		};
		this.resultTabs = [...this.resultTabs];
	}

	private async applyPageReady(
		event: Extract<QueryEvent, { type: 'page_ready' }>,
		generation: number
	) {
		const tabIndex = this.resultTabs.findIndex((t) => t.queryId === event.query_id);
		if (tabIndex < 0) return;

		this.resultTabs[tabIndex] = {
			...this.resultTabs[tabIndex],
			totalPages: event.page_count
		};
		this.resultTabs = [...this.resultTabs];

		if (event.page_index === 0 && !this.resultTabs[tabIndex].currentPageData) {
			await this.loadPage(event.query_id, 0);
			if (generation !== this.generation) return;
		}
	}

	private applyFinished(event: Extract<QueryEvent, { type: 'finished' }>) {
		const tabIndex = this.resultTabs.findIndex((t) => t.queryId === event.query_id);
		if (tabIndex < 0) return;

		const tab = this.resultTabs[tabIndex];

		this.resultTabs[tabIndex] = {
			...tab,
			status: event.status,
			error: event.error ?? undefined,
			queryReturnsResults: event.affected_rows == null ? tab.queryReturnsResults : false,
			affectedRows: event.affected_rows ?? tab.affectedRows
		};
		this.resultTabs = [...this.resultTabs];

		if (event.status === 'Completed' && this.trackedQueries.get(event.query_id) !== 'Completed') {
			this.trackedQueries.set(event.query_id, 'Completed');
			if (event.affected_rows != null) {
				this.onComplete?.(event.affected_rows);
			} else {
				this.onComplete?.((tab.totalPages || 0) * 50);
			}
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
