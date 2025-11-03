<script lang="ts">
	import { ResizablePaneGroup, ResizablePane, ResizableHandle } from '$lib/components/ui/resizable';
	import { Card, CardContent } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { ChevronLeft, ChevronRight } from '@lucide/svelte';
	import Table from './Table.svelte';
	import JsonInspector from './JsonInspector.svelte';
	import TabBar from '$lib/components/ui/TabBar.svelte';
	import {
		Commands,
		type ConnectionInfo,
		type Script,
		type Json,
		type QueryId,
		type Page,
		type QueryStatus,
		type StatementInfo
	} from '$lib/commands.svelte';
	import { createEditor } from '$lib/codemirror';
	import { onMount } from 'svelte';
	import { EditorState } from '@codemirror/state';

	interface Props {
		selectedConnection: string | null;
		connections: ConnectionInfo[];
		currentScript: Script | null;
		hasUnsavedChanges: boolean;
		onContentChange?: (content: string) => void;
		onLoadFromHistory?: (historyQuery: string) => void;
		onHistoryUpdate?: () => void;
	}

	let {
		selectedConnection = $bindable(),
		connections = $bindable(),
		currentScript = $bindable(),
		hasUnsavedChanges = $bindable(),
		onContentChange,
		onLoadFromHistory,
		onHistoryUpdate
	}: Props = $props();

	let editorContainer = $state<HTMLElement>();
	let sqlEditor: ReturnType<typeof createEditor> | null = null;

	let sqlQuery = $state('');

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
		totalPages: number | null; // null = unknown (still running), number = known (complete)
		error?: string;
	}

	let resultTabs = $state<QueryResultTab[]>([]);
	let activeResultTabId = $state<number | null>(null);
	// Counter for result tab IDs
	let nextResultTabId = $state(1);

	// Query execution state
	let currentQuery = $state<string>('');

	let pageCountPolls = $state<Map<QueryId, ReturnType<typeof setInterval>>>(new Map());

	const isConnected = $derived.by(() => {
		if (!selectedConnection) return false;
		const connection = connections.find((c) => c.id === selectedConnection);
		return connection?.connected || false;
	});

	export function getContent(): string {
		return sqlQuery;
	}

	export function setContent(content: string) {
		sqlQuery = content;
		onContentChange?.(content);
		if (sqlEditor) {
			sqlEditor.updateValue(content);
		}
	}

	// Set content without onContentChange
	export function setContentSilently(content: string) {
		sqlQuery = content;
		if (sqlEditor) {
			sqlEditor.updateValue(content);
		}
	}

	export function saveState(): EditorState | undefined {
		return sqlEditor?.saveState();
	}

	export function restoreState(state: EditorState) {
		if (sqlEditor && state) {
			sqlEditor.restoreState(state);
			sqlQuery = sqlEditor.view.state.doc.toString();
			onContentChange?.(sqlQuery);
		}
	}

	export async function handleExecuteQuery(queryToExecute?: string) {
		const query = queryToExecute || sqlQuery;
		if (!selectedConnection || !query.trim()) return;

		if (!isConnected) {
			// TODO(vini): make this into a toast
			console.warn('Cannot execute query: No active database connection');
			return;
		}

		currentQuery = query.trim();

		for (const interval of pageCountPolls.values()) {
			clearInterval(interval);
		}
		pageCountPolls.clear();
		resultTabs = [];
		activeResultTabId = null;

		try {
			const queryIds = await Commands.submitQuery(selectedConnection, currentQuery);
			console.log('Received queryIds:', queryIds);

			// create a tab per statement
			for (const queryId of queryIds) {
				await initializeQueryTab(queryId);
			}
		} catch (error) {
			console.error('Failed to execute query:', error);

			const errorObj = error as { message: string };

			const tabId = nextResultTabId++;

			const errorTab: QueryResultTab = {
				id: tabId,
				queryId: -1,
				name: generateTabTitle(currentQuery),
				query: currentQuery,
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

	let currentTableBrowse: { tableName: string; schema: string } | null = $state(null);

	export function loadQueryFromHistory(historyQuery: string) {
		if (onLoadFromHistory) {
			onLoadFromHistory(historyQuery);
		}
	}

	export function handleTableBrowse(tableName: string, schema: string) {
		if (!selectedConnection || !tableName) return;

		if (!isConnected) {
			console.warn('Cannot browse table: No active database connection');
			return;
		}

		const query =
			!schema || schema === 'public'
				? `SELECT * FROM "${tableName}" LIMIT 1000`
				: `SELECT * FROM "${schema}"."${tableName}" LIMIT 1000`;

		currentTableBrowse = { tableName, schema };

		console.log('Table browse started for:', tableName, 'with query:', query);

		handleExecuteQuery(query);
	}

	async function initializeQueryTab(queryId: QueryId) {
		const tabId = nextResultTabId++;

		const newTab: QueryResultTab = {
			id: tabId,
			queryId,
			name: `Query ${queryId}`,
			query: currentQuery,
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

		let tabName = generateTabTitle(currentQuery);
		if (currentTableBrowse) {
			const tableDisplayName =
				!currentTableBrowse.schema || currentTableBrowse.schema === 'public'
					? currentTableBrowse.tableName
					: `${currentTableBrowse.schema}.${currentTableBrowse.tableName}`;
			tabName = `📋 ${tableDisplayName}`;
			currentTableBrowse = null;
		}

		if (!info.returns_values) {
			resultTabs[tabIndex] = {
				...resultTabs[tabIndex],
				status: info.status,
				name: tabName,
				queryReturnsResults: false,
				affectedRows: info.affected_rows ?? undefined
			};
			resultTabs = [...resultTabs];

			if (info.status === 'Completed' && selectedConnection) {
				Commands.saveQueryToHistory(
					selectedConnection,
					currentQuery,
					undefined,
					'success',
					info.affected_rows ?? 0,
					undefined
				);
				onHistoryUpdate?.();
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

	// TODO(vini): replace with channel/listener
	async function pollForColumns(queryId: QueryId): Promise<string[] | null> {
		// Poll for up to 5 seconds
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

	// TODO(vini): drop this for a channel
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
					// Reactivity trigger
					resultTabs = [...resultTabs];
				}

				if (status === 'Completed' && selectedConnection) {
					const interval = pageCountPolls.get(queryId);
					if (interval) {
						clearInterval(interval);
						pageCountPolls.delete(queryId);
					}

					const totalRows = (tab.totalPages || 0) * 50;

					Commands.saveQueryToHistory(
						selectedConnection,
						tab.query,
						undefined,
						'success',
						totalRows,
						undefined
					);
					onHistoryUpdate?.();
				}
			} catch (error) {
				console.error('Error polling for page count:', error);
			}
		};

		// Poll right now, then at every 200ms
		poll();
		const interval = setInterval(poll, 200);
		pageCountPolls.set(queryId, interval);
	}

	async function loadPage(queryId: QueryId, pageIndex: number) {
		const tabIndex = resultTabs.findIndex((t) => t.queryId === queryId);
		if (tabIndex < 0) return;

		console.log('Loading page', pageIndex, 'for queryId', queryId);

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

	export function handleExecuteQueryStream(queryToExecute?: string) {
		return handleExecuteQuery(queryToExecute);
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

	async function loadDatabaseSchema() {
		if (!selectedConnection || !sqlEditor) return;

		try {
			const connection = connections.find((c) => c.id === selectedConnection);
			if (connection?.connected) {
				// Get schema information for autocomplete
				const schema = await Commands.getDatabaseSchema(selectedConnection);
				sqlEditor.updateSchema(schema);
			}
		} catch (error) {
			console.error('Failed to load database schema:', error);
		}
	}

	$effect(() => {
		if (selectedConnection) {
			loadDatabaseSchema();
		}
	});

	onMount(() => {
		const initializeEditor = () => {
			if (editorContainer && editorContainer.offsetParent !== null) {
				sqlEditor = createEditor({
					container: editorContainer,
					value: sqlQuery,
					onChange: (newValue) => {
						sqlQuery = newValue;
						onContentChange?.(newValue);
					},
					onExecute: handleExecuteQuery,
					onExecuteSelection: (selectedText: string) => {
						handleExecuteQuery(selectedText);
					},
					disabled: false,
					schema: null
				});

				loadDatabaseSchema();
			} else {
				setTimeout(initializeEditor, 100);
			}
		};

		initializeEditor();
	});
</script>

<div class="flex flex-1 flex-col">
	<ResizablePaneGroup direction="vertical" class="flex-1">
		<ResizablePane defaultSize={60} minSize={30} maxSize={80}>
			<div class="h-full">
				<Card class="flex h-full flex-col gap-0 overflow-hidden rounded-none border-none py-0">
					<CardContent class="min-h-0 flex-1 p-0">
						<div bind:this={editorContainer} class="h-full w-full"></div>
					</CardContent>
				</Card>
			</div>
		</ResizablePane>

		<ResizableHandle />

		<ResizablePane defaultSize={40} minSize={20}>
			<div class="relative flex h-full flex-col">
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

				<Card class="flex flex-1 flex-col gap-0 overflow-hidden rounded-none border-none pt-0 pb-0">
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
													onclick={() =>
														loadPage(activeTab.queryId, activeTab.currentPageIndex - 1)}
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
														onclick={() =>
															loadPage(activeTab.queryId, activeTab.currentPageIndex - 1)}
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
														onclick={() =>
															loadPage(activeTab.queryId, activeTab.currentPageIndex + 1)}
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
													onclick={() =>
														loadPage(activeTab.queryId, activeTab.currentPageIndex + 1)}
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
								<div class="space-y-3 text-xs">
									<div class="flex min-w-[20rem] items-center justify-between gap-4">
										<span class="text-muted-foreground/80">Run selected text or current line</span>
										<kbd
											class="bg-muted text-muted-foreground pointer-events-none inline-flex h-5 items-center justify-center gap-1 rounded border px-1.5 font-mono text-[10px] font-medium opacity-100 select-none"
										>
											<span class="text-xs">⌘</span>Enter
										</kbd>
									</div>
									<div class="flex min-w-[20rem] items-center justify-between gap-4">
										<span class="text-muted-foreground/80">Run entire script</span>
										<kbd
											class="bg-muted text-muted-foreground pointer-events-none inline-flex h-5 items-center justify-center gap-1 rounded border px-1.5 font-mono text-[10px] font-medium opacity-100 select-none"
										>
											<span class="text-xs">⌘</span>R
										</kbd>
									</div>
								</div>
							</div>
						</CardContent>
					{/if}
				</Card>
			</div>
		</ResizablePane>
	</ResizablePaneGroup>
</div>
