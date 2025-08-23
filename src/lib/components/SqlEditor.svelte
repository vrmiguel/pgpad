<script lang="ts">
	import { ResizablePaneGroup, ResizablePane, ResizableHandle } from '$lib/components/ui/resizable';
	import { Card, CardHeader, CardContent } from '$lib/components/ui/card';
	import QueryResultsTable from './QueryResultsTable.svelte';
	import StatementExecutor from './StatementExecutor.svelte';
	import TabBar from '$lib/components/ui/TabBar.svelte';
	import {
		Commands,
		type ConnectionInfo,
		type Script,
		type QueryHistoryEntry
	} from '$lib/commands.svelte';
	import { createEditor } from '$lib/codemirror';
	import { onMount } from 'svelte';
	import { Table, History } from '@lucide/svelte';

	interface Props {
		selectedConnection: string | null;
		connections: ConnectionInfo[];
		currentScript: Script | null;
		hasUnsavedChanges: boolean;
		onContentChange?: (content: string) => void;
		onLoadFromHistory?: (historyQuery: string) => void;
	}

	let {
		selectedConnection = $bindable(),
		connections = $bindable(),
		currentScript = $bindable(),
		hasUnsavedChanges = $bindable(),
		onContentChange,
		onLoadFromHistory
	}: Props = $props();

	let editorContainer = $state<HTMLElement>();
	let sqlEditor: ReturnType<typeof createEditor> | null = null;

	let sqlQuery = $state(`-- Welcome to PgPad!
-- Keyboard shortcuts:
--   Ctrl+Enter: Run selected text (or current line if nothing selected)
--   Ctrl+R: Run entire script

SELECT 1 as test;`);

	let queryHistory = $state<QueryHistoryEntry[]>([]);

	interface QueryResultTab {
		id: string;
		name: string;
		query: string;
		timestamp: number;
		status: 'running' | 'completed' | 'error';
		statementIndex?: number;
		queryReturnsResults?: boolean;
		affectedRows?: number;
		columns?: string[];
		rows?: any[][];
		error?: string;
	}

	let resultTabs = $state<QueryResultTab[]>([]);
	let activeResultTabId = $state<string | null>(null);
	let showHistory = $state(true);

	// Query execution state
	let currentQuery = $state<string>('');

	const isConnected = $derived(() => {
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

	async function loadQueryHistory() {
		if (!selectedConnection) return;

		try {
			queryHistory = await Commands.getQueryHistory(selectedConnection, 50);
		} catch (error) {
			console.error('Failed to load query history:', error);
			queryHistory = [];
		}
	}

	function handleQueryComplete(rowCount: number, duration: number) {
		if (activeResultTabId) {
			const tab = resultTabs.find((t) => t.id === activeResultTabId);
			if (tab) tab.status = 'completed';
		}

		if (selectedConnection) {
			Commands.saveQueryToHistory(
				selectedConnection,
				currentQuery,
				duration,
				'success',
				rowCount,
				undefined
			);
			loadQueryHistory();
		}
	}

	function handleQueryError(error: string) {
		if (activeResultTabId) {
			const tab = resultTabs.find((t) => t.id === activeResultTabId);
			if (tab) tab.status = 'error';
		}

		if (selectedConnection) {
			Commands.saveQueryToHistory(selectedConnection, currentQuery, 0, 'error', 0, error);
			loadQueryHistory();
		}
	}

	let executionTrigger = $state(0);

	export function handleExecuteQuery(queryToExecute?: string) {
		const query = queryToExecute || sqlQuery;
		if (!selectedConnection || !query.trim()) return;

		if (!isConnected) {
			console.warn('Cannot execute query: No active database connection');
			return;
		}

		currentQuery = query.trim();

		resultTabs = [];
		activeResultTabId = null;

		// Trigger new execution
		executionTrigger++;

		console.log('HandleExecuteQuery called with:', currentQuery, 'trigger:', executionTrigger);

		showHistory = false;
	}

	function handleStatementStart(
		statementIndex: number,
		statement: string,
		returnsValues: boolean
	): string {
		const tabId = crypto.randomUUID();

		if (returnsValues) {
			const newTab: QueryResultTab = {
				id: tabId,
				name: generateTabTitle(statement),
				query: statement,
				timestamp: Date.now(),
				status: 'running',
				statementIndex
			};

			resultTabs.push(newTab);
			activeResultTabId = tabId;
		} else {
			const newTab: QueryResultTab = {
				id: tabId,
				name: generateTabTitle(statement),
				query: statement,
				timestamp: Date.now(),
				status: 'running',
				statementIndex,
				queryReturnsResults: true
			};

			resultTabs.push(newTab);
			activeResultTabId = tabId;
		}

		return tabId;
	}

	function handleStatementComplete(tabId: string, rowCount: number, duration: number) {
		const tab = resultTabs.find((t) => t.id === tabId);
		if (tab) {
			if (tab.queryReturnsResults) {
				tab.affectedRows = rowCount;
			}
			tab.status = 'completed';
		}

		if (selectedConnection && tab) {
			Commands.saveQueryToHistory(
				selectedConnection,
				tab.query,
				duration,
				'success',
				rowCount,
				undefined
			);
			loadQueryHistory();
		}
	}

	// Handle when a statement has an error
	function handleStatementError(tabId: string, error: string) {
		const tab = resultTabs.find((t) => t.id === tabId);
		if (tab) {
			tab.status = 'error';
			tab.error = error;
		}

		// Save to history
		if (selectedConnection && tab) {
			Commands.saveQueryToHistory(selectedConnection, tab.query, 0, 'error', 0, error);
			loadQueryHistory();
		}
	}

	// Handle tab updates from StatementExecutor
	function handleTabUpdate(tabId: string, updates: Partial<QueryResultTab>) {
		const tabIndex = resultTabs.findIndex((t) => t.id === tabId);
		if (tabIndex !== -1) {
			const tab = resultTabs[tabIndex];

			// Handle row batching specially
			if (updates.rows && tab.rows) {
				// Append new rows to existing ones
				tab.rows = [...tab.rows, ...updates.rows];
			} else {
				// Apply other updates
				Object.assign(tab, updates);
			}
		}
	}

	export function handleExecuteQueryStream(queryToExecute?: string) {
		return handleExecuteQuery(queryToExecute);
	}

	function loadQueryFromHistory(historyQuery: string) {
		if (onLoadFromHistory) {
			onLoadFromHistory(historyQuery);
		}
	}

	function formatDuration(ms: number | null): string {
		if (ms === null || ms === undefined) return '0ms';
		if (ms < 1000) return `${ms}ms`;
		return `${(ms / 1000).toFixed(2)}s`;
	}

	function formatTimestamp(timestamp: number): string {
		return new Date(timestamp * 1000).toLocaleString();
	}

	function generateTabTitle(query: string): string {
		const cleaned = query.trim().replace(/\s+/g, ' ');
		if (cleaned.length <= 30) return cleaned;
		return cleaned.substring(0, 27) + '...';
	}

	function handleResultTabClose(tabId: string | number) {
		if (tabId === 'history') return;

		resultTabs = resultTabs.filter((tab) => tab.id !== tabId);

		if (activeResultTabId === tabId) {
			activeResultTabId = resultTabs.length > 0 ? resultTabs[0].id : null;
		}
	}

	function handleResultTabSelect(tabId: string | number) {
		if (tabId === 'history') {
			showHistory = true;
			activeResultTabId = null;
		} else {
			showHistory = false;
			activeResultTabId = tabId as string;
		}
	}

	function getTabStatus(tab: any): 'normal' | 'modified' | 'error' {
		if (tab.id === 'history') return 'normal';

		switch (tab.status) {
			case 'error':
				return 'error';
			case 'running':
				return 'modified';
			default:
				return 'normal';
		}
	}

	// Create combined tabs (results + history)
	const allTabs = $derived(() => {
		const tabs = [...resultTabs];
		tabs.push({
			id: 'history',
			name: `History (${queryHistory.length})`,
			query: '',
			timestamp: 0,
			status: 'completed' as const
		});
		return tabs;
	});

	// Load database schema for editor completion
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

	// Effect to reload history when connection changes
	$effect(() => {
		if (selectedConnection) {
			loadQueryHistory();
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

		if (selectedConnection) {
			loadQueryHistory();
		}
	});
</script>

<div class="flex flex-1 flex-col">
	<ResizablePaneGroup direction="vertical" class="flex-1">
		<!-- SQL Editor Pane -->
		<ResizablePane defaultSize={60} minSize={30} maxSize={80}>
			<div class="h-full p-1 pb-0.5">
				<Card class="flex h-full flex-col gap-0 py-0">
					<CardContent class="min-h-0 flex-1 p-0">
						<div bind:this={editorContainer} class="h-full w-full overflow-hidden rounded-md"></div>
					</CardContent>
				</Card>
			</div>
		</ResizablePane>

		<ResizableHandle />

		<!-- Results & History Section Pane -->
		<ResizablePane defaultSize={40} minSize={20}>
			<div class="relative h-full px-1 pt-0.5 pb-1">
				<!-- Result Tabs -->
				<div class="relative z-10">
					<TabBar
						tabs={allTabs()}
						activeTabId={showHistory ? 'history' : activeResultTabId}
						onTabSelect={handleResultTabSelect}
						onTabClose={handleResultTabClose}
						showCloseButton={true}
						showNewTabButton={false}
						allowRename={false}
						{getTabStatus}
						closeTabLabel="Close result tab"
						maxTabWidth="max-w-64"
						variant="seamless"
					/>
				</div>

				<Card
					class="-mt-px flex h-full min-h-0 flex-col gap-0 overflow-hidden rounded-t-none border-t-0 pt-0 pb-2"
				>
					{#if showHistory}
						<CardContent class="flex min-h-0 flex-1 flex-col px-6 pt-0 pb-6">
							{#if queryHistory.length > 0}
								<div class="flex-1 overflow-auto">
									<div class="space-y-2 p-2">
										{#each queryHistory as historyItem}
											<button
												type="button"
												class="group hover:bg-muted/30 w-full cursor-pointer rounded-lg border p-3 text-left transition-colors"
												onclick={() => loadQueryFromHistory(historyItem.query_text)}
											>
												<div class="mb-2 flex items-start justify-between gap-2">
													<div class="flex min-w-0 flex-1 items-center gap-2">
														<div class="flex items-center gap-1">
															{#if historyItem.status === 'success'}
																<div class="h-2 w-2 rounded-full bg-green-500"></div>
															{:else}
																<div class="h-2 w-2 rounded-full bg-red-500"></div>
															{/if}
														</div>
														<span class="text-muted-foreground text-xs">
															{formatTimestamp(historyItem.executed_at)}
														</span>
														{#if historyItem.status === 'success'}
															<span class="text-muted-foreground text-xs">
																{historyItem.row_count} rows
															</span>
														{/if}
														<span class="text-muted-foreground text-xs">
															{formatDuration(historyItem.duration_ms)}
														</span>
													</div>
													<span
														class="text-primary text-xs font-medium opacity-0 group-hover:opacity-100"
													>
														Load
													</span>
												</div>
												<code
													class="bg-muted/50 block overflow-hidden rounded p-2 text-left text-xs"
												>
													{historyItem.query_text.length > 200
														? historyItem.query_text.slice(0, 200) + '...'
														: historyItem.query_text}
												</code>
												{#if historyItem.error_message}
													<p class="mt-1 rounded bg-red-50 p-2 text-left text-xs text-red-600">
														{historyItem.error_message}
													</p>
												{/if}
											</button>
										{/each}
									</div>
								</div>
							{:else}
								<div class="text-muted-foreground flex flex-1 items-center justify-center">
									<div class="text-center">
										<div
											class="bg-muted/20 mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full"
										>
											<History class="text-muted-foreground/50 h-8 w-8" />
										</div>
										<p class="text-sm font-medium">No query history</p>
										<p class="text-muted-foreground/70 mt-1 text-xs">
											Execute queries to see history here
										</p>
									</div>
								</div>
							{/if}
						</CardContent>
					{:else if activeResultTabId}
						{@const activeTab = resultTabs.find((t) => t.id === activeResultTabId)}
						{#if activeTab}
							<CardContent
								class="flex h-full min-h-0 flex-1 flex-col overflow-hidden {activeTab.columns &&
								activeTab.rows &&
								activeTab.rows.length > 0
									? 'p-0'
									: 'px-6 pb-6'}"
							>
								{#if activeTab.error}
									<!-- Error state -->
									<div class="flex h-full flex-1 items-center justify-center">
										<div class="text-center">
											<div class="text-sm text-red-600">❌ {activeTab.error}</div>
										</div>
									</div>
								{:else if activeTab.queryReturnsResults}
									<!-- Modification query result -->
									<div class="flex h-full flex-1 items-center justify-center">
										<div class="text-center">
											{#if activeTab.status === 'running'}
												<div class="text-muted-foreground text-sm">
													Executing modification query...
												</div>
											{:else if activeTab.status === 'completed'}
												<div class="text-sm font-medium text-green-600">
													✓ {activeTab.affectedRows || 0} rows affected
												</div>
											{/if}
										</div>
									</div>
								{:else if activeTab.status === 'running'}
									<!-- Loading state for SELECT queries -->
									<div class="flex h-full flex-1 items-center justify-center">
										<div class="text-center">
											<div class="text-muted-foreground text-sm">Loading results...</div>
										</div>
									</div>
								{:else if activeTab.columns && activeTab.rows}
									<!-- SELECT query results -->
									{#if activeTab.rows.length > 0}
										<QueryResultsTable data={activeTab.rows} columns={activeTab.columns} />
									{:else}
										<div class="flex h-full flex-1 items-center justify-center">
											<div class="text-center">
												<div class="text-muted-foreground text-sm">No rows returned.</div>
											</div>
										</div>
									{/if}
								{/if}
							</CardContent>
						{/if}
					{:else}
						<CardContent class="flex h-full min-h-0 flex-1 flex-col overflow-hidden px-6 pt-0 pb-6">
							<div class="text-muted-foreground flex flex-1 items-center justify-center">
								<div class="text-center">
									<div
										class="bg-muted/20 mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full"
									>
										<Table class="text-muted-foreground/50 h-8 w-8" />
									</div>
									<p class="text-sm font-medium">No results to display</p>
									<p class="text-muted-foreground/70 mt-1 text-xs">
										Run a query to see results here
									</p>
								</div>
							</div>
						</CardContent>
					{/if}
				</Card>
			</div>
		</ResizablePane>
	</ResizablePaneGroup>

	{#if currentQuery && selectedConnection}
		<StatementExecutor
			connectionId={selectedConnection}
			query={currentQuery}
			{executionTrigger}
			onStatementStart={handleStatementStart}
			onStatementComplete={handleStatementComplete}
			onStatementError={handleStatementError}
			onTabUpdate={handleTabUpdate}
		/>
	{/if}
</div>
