<script lang="ts">
	import { Card, CardContent } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { ChevronLeft, ChevronRight } from '@lucide/svelte';
	import Table from './Table.svelte';
	import JsonInspector from './JsonInspector.svelte';
	import TabBar from '$lib/components/ui/TabBar.svelte';
	import {
		Commands,
		type Json,
		type QueryId,
		type Page,
		type QueryStatus,
		type StatementInfo
	} from '$lib/commands.svelte';

	interface Props {
		/** The SQL query to execute */
		query: string;
		/** Connection ID to execute against */
		connectionId: string;
		/** Callback when query completes successfully */
		onQueryComplete?: (totalRows: number) => void;
		/** Whether to show result tabs (for multi-statement queries) */
		showResultTabs?: boolean;
	}

	let { query, connectionId, onQueryComplete, showResultTabs = true }: Props = $props();

	let selectedCellData = $state<Json | null>(null);
	let jsonInspectorData = $state<{ data: Json; position: { x: number; y: number } } | null>(null);

	interface QueryResultTab {
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

	let resultTabs = $state<QueryResultTab[]>([]);
	let activeResultTabId = $state<number | null>(null);
	let nextResultTabId = $state(1);
	let pageCountPolls = $state<Map<QueryId, ReturnType<typeof setInterval>>>(new Map());

	// Execute query when props change
	$effect(() => {
		if (query && connectionId) {
			executeQuery(query);
		}
	});

	async function executeQuery(queryText: string) {
		// Clear previous results
		for (const interval of pageCountPolls.values()) {
			clearInterval(interval);
		}
		pageCountPolls.clear();
		resultTabs = [];
		activeResultTabId = null;
		nextResultTabId = 1;

		try {
			const queryIds = await Commands.submitQuery(connectionId, queryText.trim());

			// Create a tab per statement
			for (const queryId of queryIds) {
				await initializeQueryTab(queryId, queryText);
			}
		} catch (error) {
			console.error('Failed to execute query:', error);

			const errorObj = error as { message: string };
			const tabId = nextResultTabId++;

			const errorTab: QueryResultTab = {
				id: tabId,
				queryId: -1,
				name: generateTabTitle(queryText),
				query: queryText,
				timestamp: Date.now(),
				status: 'Error',
				currentPageIndex: 0,
				currentPageData: null,
				totalPages: null,
				error: errorObj.message
			};

			resultTabs = [errorTab];
			activeResultTabId = tabId;
		}
	}

	async function initializeQueryTab(queryId: QueryId, queryText: string) {
		const tabId = nextResultTabId++;

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

		resultTabs = [...resultTabs, newTab];
		activeResultTabId = tabId;

		const info = await waitUntilRenderable(queryId);

		const tabIndex = resultTabs.findIndex((t) => t.id === tabId);
		if (tabIndex < 0) return;

		if (info.error) {
			resultTabs[tabIndex] = {
				...resultTabs[tabIndex],
				status: 'Error',
				error: info.error
			};
			resultTabs = [...resultTabs];
			return;
		}

		const tabName = generateTabTitle(queryText);

		if (!info.returns_values) {
			resultTabs[tabIndex] = {
				...resultTabs[tabIndex],
				status: info.status,
				name: tabName,
				queryReturnsResults: false,
				affectedRows: info.affected_rows ?? undefined
			};
			resultTabs = [...resultTabs];

			if (info.status === 'Completed') {
				onQueryComplete?.(info.affected_rows ?? 0);
			}

			return;
		}

		const columns = await pollForColumns(queryId);
		if (!columns) {
			resultTabs[tabIndex] = {
				...resultTabs[tabIndex],
				status: 'Error',
				error: 'Failed to get column information'
			};
			resultTabs = [...resultTabs];
			return;
		}

		resultTabs[tabIndex] = {
			...resultTabs[tabIndex],
			columns,
			name: tabName,
			currentPageData: info.first_page,
			status: info.status,
			queryReturnsResults: true
		};
		resultTabs = [...resultTabs];

		if (info.status === 'Completed') {
			const pageCount = await Commands.getPageCount(queryId);
			const tabIdx = resultTabs.findIndex((t) => t.id === tabId);
			if (tabIdx >= 0) {
				resultTabs[tabIdx] = { ...resultTabs[tabIdx], totalPages: pageCount };
				resultTabs = [...resultTabs];
			}

			onQueryComplete?.((pageCount || 0) * 50);
		} else {
			pollForPageCount(queryId);
		}
	}

	async function waitUntilRenderable(queryId: QueryId): Promise<StatementInfo> {
		const now = performance.now();
		const res = await Commands.waitUntilRenderable(queryId);
		const elapsed = performance.now() - now;
		console.log('Wait until renderable took', elapsed, 'ms');
		return res;
	}

	async function pollForColumns(queryId: QueryId): Promise<string[] | null> {
		for (let i = 0; i < 50; i++) {
			const columns = await Commands.getColumns(queryId);
			if (columns) {
				return columns;
			}
			await new Promise((resolve) => setTimeout(resolve, 100));
		}
		return null;
	}

	async function pollForPage(queryId: QueryId, pageIndex: number): Promise<Page | null> {
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

	function pollForPageCount(queryId: QueryId) {
		const poll = async () => {
			try {
				const status = await Commands.getQueryStatus(queryId);
				const pageCount = await Commands.getPageCount(queryId);

				const tab = resultTabs.find((t) => t.queryId === queryId);
				if (!tab) return;

				let changed = false;

				if (tab.totalPages !== pageCount) {
					const tabIndex = resultTabs.findIndex((t) => t.queryId === queryId);
					if (tabIndex >= 0) {
						resultTabs[tabIndex] = { ...resultTabs[tabIndex], totalPages: pageCount };
						changed = true;
					}
				}

				if (tab.status !== status) {
					const tabIndex = resultTabs.findIndex((t) => t.queryId === queryId);
					if (tabIndex >= 0) {
						resultTabs[tabIndex] = { ...resultTabs[tabIndex], status };
						changed = true;
					}
				}

				if (changed) {
					resultTabs = [...resultTabs];
				}

				if (status === 'Completed') {
					const interval = pageCountPolls.get(queryId);
					if (interval) {
						clearInterval(interval);
						pageCountPolls.delete(queryId);
					}

					const totalRows = (tab.totalPages || 0) * 50;
					onQueryComplete?.(totalRows);
				}
			} catch (error) {
				console.error('Error polling for page count:', error);
			}
		};

		poll();
		const interval = setInterval(poll, 200);
		pageCountPolls.set(queryId, interval);
	}

	async function loadPage(queryId: QueryId, pageIndex: number) {
		const tabIndex = resultTabs.findIndex((t) => t.queryId === queryId);
		if (tabIndex < 0) return;

		const page = await pollForPage(queryId, pageIndex);
		if (page) {
			resultTabs[tabIndex] = {
				...resultTabs[tabIndex],
				currentPageIndex: pageIndex,
				currentPageData: page
			};
			resultTabs = [...resultTabs];
		}
	}

	function generateTabTitle(query: string): string {
		const cleaned = query.trim().replace(/\s+/g, ' ');
		if (cleaned.length <= 30) return cleaned;
		return cleaned.substring(0, 27) + '...';
	}

	function handleResultTabClose(tabId: number) {
		resultTabs = resultTabs.filter((tab) => tab.id !== tabId);

		if (activeResultTabId === tabId) {
			activeResultTabId = resultTabs.length > 0 ? resultTabs[0].id : null;
		}
	}

	function handleResultTabSelect(tabId: number) {
		activeResultTabId = tabId;
	}

	function getTabStatus(tab: QueryResultTab): 'normal' | 'modified' | 'error' {
		switch (tab.status) {
			case 'Error':
				return 'error';
			case 'Running':
				return 'modified';
			default:
				return 'normal';
		}
	}
</script>

<div class="relative flex h-full flex-col">
	{#if showResultTabs && resultTabs.length > 0}
		<div class="relative z-10">
			<TabBar
				tabs={resultTabs}
				activeTabId={activeResultTabId}
				onTabSelect={handleResultTabSelect}
				onTabClose={handleResultTabClose}
				onNewTab={undefined}
				onTabRename={undefined}
				showCloseButton={true}
				showNewTabButton={false}
				allowRename={false}
				{getTabStatus}
				closeTabLabel="Close result tab"
				maxTabWidth="max-w-64"
				variant="default"
			/>
		</div>
	{/if}

	<Card
		class="flex flex-1 flex-col gap-0 overflow-hidden rounded-none border-none {showResultTabs
			? 'pt-0'
			: ''} pb-0"
	>
		{#if activeResultTabId}
			{@const activeTab = resultTabs.find((t) => t.id === activeResultTabId)}
			{#if activeTab}
				{#if activeTab.columns && activeTab.currentPageData && activeTab.currentPageData.length > 0}
					<div class="relative flex min-h-0 flex-1 flex-col">
						<CardContent class="flex h-full min-h-0 flex-1 flex-col overflow-hidden p-0">
							<Table
								data={activeTab.currentPageData}
								columns={activeTab.columns}
								bind:selectedCellData
								onJsonInspect={(data, position) => {
									jsonInspectorData = { data, position };
								}}
							/>
						</CardContent>

						{#if activeTab.totalPages && activeTab.totalPages > 1}
							<div
								class="border-border/30 bg-muted/20 flex flex-shrink-0 items-center border-t px-3 py-2"
							>
								<div class="text-muted-foreground flex items-center gap-2 text-xs">
									<span>Page {activeTab.currentPageIndex + 1} of {activeTab.totalPages}</span>
								</div>

								<div class="flex-1"></div>

								<div class="flex items-center gap-1">
									<Button
										variant="ghost"
										size="sm"
										onclick={() => loadPage(activeTab.queryId, activeTab.currentPageIndex - 1)}
										disabled={activeTab.currentPageIndex === 0}
										class="h-6 w-6 p-0"
									>
										<ChevronLeft class="h-3 w-3" />
									</Button>

									{#if activeTab.currentPageIndex > 1}
										<Button
											variant="ghost"
											size="sm"
											onclick={() => loadPage(activeTab.queryId, 0)}
											class="h-6 px-2 text-xs"
										>
											1
										</Button>
										{#if activeTab.currentPageIndex > 2}
											<span class="text-muted-foreground text-xs">...</span>
										{/if}
									{/if}

									{#if activeTab.currentPageIndex > 0}
										<Button
											variant="ghost"
											size="sm"
											onclick={() => loadPage(activeTab.queryId, activeTab.currentPageIndex - 1)}
											class="h-6 px-2 text-xs"
										>
											{activeTab.currentPageIndex}
										</Button>
									{/if}

									<Button variant="default" size="sm" class="h-6 px-2 text-xs">
										{activeTab.currentPageIndex + 1}
									</Button>

									{#if activeTab.currentPageIndex < activeTab.totalPages - 1}
										<Button
											variant="ghost"
											size="sm"
											onclick={() => loadPage(activeTab.queryId, activeTab.currentPageIndex + 1)}
											class="h-6 px-2 text-xs"
										>
											{activeTab.currentPageIndex + 2}
										</Button>
									{/if}

									{#if activeTab.totalPages && activeTab.currentPageIndex < activeTab.totalPages - 2}
										{#if activeTab.currentPageIndex < activeTab.totalPages - 3}
											<span class="text-muted-foreground text-xs">...</span>
										{/if}
										<Button
											variant="ghost"
											size="sm"
											onclick={() =>
												activeTab.totalPages &&
												loadPage(activeTab.queryId, activeTab.totalPages - 1)}
											class="h-6 px-2 text-xs"
										>
											{activeTab.totalPages}
										</Button>
									{/if}

									<Button
										variant="ghost"
										size="sm"
										onclick={() => loadPage(activeTab.queryId, activeTab.currentPageIndex + 1)}
										disabled={activeTab.currentPageIndex >= activeTab.totalPages - 1}
										class="h-6 w-6 p-0"
									>
										<ChevronRight class="h-3 w-3" />
									</Button>
								</div>
							</div>
						{/if}

						{#if jsonInspectorData}
							<JsonInspector
								selectedCellData={jsonInspectorData.data}
								initialPosition={jsonInspectorData.position}
								onClose={() => {
									jsonInspectorData = null;
								}}
							/>
						{/if}
					</div>
				{:else}
					<CardContent class="flex h-full min-h-0 flex-1 flex-col overflow-hidden px-6">
						{#if activeTab.error}
							<div class="flex h-full flex-1 items-center justify-center">
								<div class="text-center">
									<div class="text-sm text-red-600">{activeTab.error}</div>
								</div>
							</div>
						{:else if activeTab.queryReturnsResults === false}
							<div class="flex h-full flex-1 items-center justify-center">
								<div class="text-center">
									{#if activeTab.status === 'Running'}
										<div class="text-muted-foreground text-sm">Executing query...</div>
									{:else if activeTab.status === 'Completed'}
										<div class="text-sm font-medium text-green-600">
											✓ {activeTab.affectedRows || 0} rows affected
										</div>
									{/if}
								</div>
							</div>
						{:else if activeTab.status === 'Running'}
							<div class="flex h-full flex-1 items-center justify-center">
								<div class="text-center">
									<div class="text-muted-foreground text-sm">Loading results...</div>
								</div>
							</div>
						{:else if activeTab.columns && activeTab.status === 'Completed' && (!activeTab.currentPageData || activeTab.currentPageData.length === 0)}
							<div class="flex h-full flex-1 items-center justify-center">
								<div class="text-center">
									<div class="text-muted-foreground text-sm">No rows returned.</div>
								</div>
							</div>
						{/if}
					</CardContent>
				{/if}
			{/if}
		{:else}
			<CardContent class="flex h-full min-h-0 flex-1 flex-col overflow-hidden px-6 pt-0">
				<div class="text-muted-foreground flex flex-1 items-center justify-center">
					<div class="text-sm">Executing query...</div>
				</div>
			</CardContent>
		{/if}
	</Card>
</div>
