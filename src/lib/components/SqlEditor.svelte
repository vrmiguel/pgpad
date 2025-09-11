<script lang="ts">
	import { ResizablePaneGroup, ResizablePane, ResizableHandle } from '$lib/components/ui/resizable';
	import { Card, CardContent } from '$lib/components/ui/card';
	import Table from './Table.svelte';
	import JsonInspector from './JsonInspector.svelte';
	import StatementExecutor from './StatementExecutor.svelte';
	import TabBar from '$lib/components/ui/TabBar.svelte';
	import {
		Commands,
		type ConnectionInfo,
		type Script,
		type Row,
		type Json
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
		name: string;
		query: string;
		timestamp: number;
		status: 'running' | 'completed' | 'error';
		statementIndex?: number;
		queryReturnsResults?: boolean;
		affectedRows?: number;
		columns?: string[];
		rows?: Row[];
		error?: string;
	}

	let resultTabs = $state<QueryResultTab[]>([]);
	let activeResultTabId = $state<number | null>(null);
	// Counter for result tab IDs
	let nextResultTabId = $state(1);

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

		resultTabs = [];
		activeResultTabId = null;
		currentQuery = query.trim();
		executionTrigger++;

		console.log('Table browse started for:', tableName, 'with query:', query);
	}

	function handleStatementStart(
		statementIndex: number,
		statement: string,
		returnsValues: boolean
	): number {
		const tabId = nextResultTabId++;

		let tabName = generateTabTitle(statement);
		if (currentTableBrowse) {
			const tableDisplayName =
				!currentTableBrowse.schema || currentTableBrowse.schema === 'public'
					? currentTableBrowse.tableName
					: `${currentTableBrowse.schema}.${currentTableBrowse.tableName}`;
			tabName = `📋 ${tableDisplayName}`;
			currentTableBrowse = null;
		}

		if (returnsValues) {
			const newTab: QueryResultTab = {
				id: tabId,
				name: tabName,
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
				name: tabName,
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

	function handleStatementComplete(tabId: number, rowCount: number, duration: number) {
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
			onHistoryUpdate?.();
		}
	}

	function handleStatementError(tabId: number, error: string) {
		const tab = resultTabs.find((t) => t.id === tabId);
		if (tab) {
			tab.status = 'error';
			tab.error = error;
		}

		if (selectedConnection && tab) {
			Commands.saveQueryToHistory(selectedConnection, tab.query, 0, 'error', 0, error);
			onHistoryUpdate?.();
		}
	}

	function handleTabUpdate(tabId: number, updates: Partial<QueryResultTab>) {
		const tabIndex = resultTabs.findIndex((t) => t.id === tabId);
		if (tabIndex !== -1) {
			const tab = resultTabs[tabIndex];

			if (updates.rows && tab.rows) {
				tab.rows.push(...updates.rows);
			} else {
				// Apply other updates
				Object.assign(tab, updates);
			}
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
			case 'error':
				return 'error';
			case 'running':
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
							{#if activeTab.columns && activeTab.rows && activeTab.rows.length > 0}
								<div class="relative flex min-h-0 flex-1">
									<CardContent class="flex h-full min-h-0 flex-1 flex-col overflow-hidden p-0">
										<Table
											data={activeTab.rows}
											columns={activeTab.columns}
											bind:selectedCellData
											onJsonInspect={(data, position) => {
												jsonInspectorData = { data, position };
											}}
										/>
									</CardContent>

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
												<div class="text-sm text-red-600">❌ {activeTab.error}</div>
											</div>
										</div>
									{:else if activeTab.queryReturnsResults}
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
										<div class="flex h-full flex-1 items-center justify-center">
											<div class="text-center">
												<div class="text-muted-foreground text-sm">Loading results...</div>
											</div>
										</div>
									{:else if activeTab.rows && activeTab.rows.length === 0}
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
