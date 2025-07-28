<script lang="ts">
	import { ResizablePaneGroup, ResizablePane, ResizableHandle } from '$lib/components/ui/resizable';
	import { Card, CardHeader, CardTitle, CardContent } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import QueryResultsTable from './QueryResultsTable.svelte';
	import { DatabaseCommands, type ConnectionInfo, type QueryResult, type QueryResultUI, type Script, type QueryHistoryEntry } from '$lib/commands.svelte';
	import { createMonacoEditor, type CreateMonacoEditorOptions } from '$lib/monaco';
	import { onMount } from 'svelte';
	import { ChevronDown, ChevronRight, Play, Loader, Table, Clock, Search, History } from '@lucide/svelte';

	interface Props {
		selectedConnection: string | null;
		connections: ConnectionInfo[];
		currentScript: Script | null;
		hasUnsavedChanges: boolean;
	}

	let { 
		selectedConnection = $bindable(), 
		connections = $bindable(), 
		currentScript = $bindable(),
		hasUnsavedChanges = $bindable()
	}: Props = $props();

	let editorContainer = $state<HTMLElement>();
	let monacoEditor: ReturnType<typeof createMonacoEditor> | null = null;

	let sqlQuery = $state(`-- Welcome to PgPad!
-- Keyboard shortcuts:
--   Ctrl+Enter: Run selected text (or current line if nothing selected)
--   Ctrl+R: Run entire script

SELECT 1 as test;`);

	let queryResultUI = $state<QueryResultUI | null>(null);
	let isExecuting = $state(false);
	let queryHistory = $state<QueryHistoryEntry[]>([]);
	let table: any = $state();
	let globalFilter = $state('');
	let activeTab = $state<'results' | 'history'>('results');

	// Derived values (safer than $effect)
	const isConnected = $derived(() => {
		if (!selectedConnection) return false;
		const connection = connections.find(c => c.id === selectedConnection);
		return connection?.connected || false;
	});

	const results = $derived(queryResultUI?.data || []);

	export function getContent(): string {
		return sqlQuery;
	}

	export function setContent(content: string) {
		sqlQuery = content;
		if (monacoEditor) {
			monacoEditor.updateValue(content);
		}
	}

	async function loadQueryHistory() {
		if (!selectedConnection) return;
		
		try {
			queryHistory = await DatabaseCommands.getQueryHistory(selectedConnection, 50);
		} catch (error) {
			console.error('Failed to load query history:', error);
			queryHistory = [];
		}
	}

	export async function handleExecuteQuery() {
		if (!selectedConnection || !sqlQuery.trim()) return;
		
		if (!isConnected()) {
			console.warn('Cannot execute query: No active database connection');
			return;
		}

		const startTime = Date.now();
		
		try {
			isExecuting = true;
			const result: QueryResult = await DatabaseCommands.executeQuery(selectedConnection, sqlQuery.trim());
			const duration = Date.now() - startTime;
			
			// Create UI-friendly result
			queryResultUI = {
				success: true,
				data: result.rows.map(row => {
					const rowObj: Record<string, any> = {};
					result.columns.forEach((col, i) => {
						rowObj[col] = row[i];
					});
					return rowObj;
				}),
				columns: result.columns,
				duration: result.duration_ms,
				queryResult: result
			};

			// Reload query history to get the newly saved query
			await loadQueryHistory();

			// Switch to results tab when query executes
			activeTab = 'results';
		} catch (error) {
			console.error('Failed to execute query:', error);
			
			queryResultUI = {
				success: false,
				data: [],
				columns: [],
				message: `Error: ${error}`,
				duration: 0
			};

			// Reload query history to get the failed query
			await loadQueryHistory();

			// Switch to results tab to show error
			activeTab = 'results';
		} finally {
			isExecuting = false;
		}
	}

	function loadQueryFromHistory(historyQuery: string) {
		sqlQuery = historyQuery;
		if (monacoEditor) {
			monacoEditor.updateValue(historyQuery);
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

	// Load database schema for Monaco completion
	async function loadDatabaseSchema() {
		if (!selectedConnection || !monacoEditor) return;
		
		try {
			const connection = connections.find(c => c.id === selectedConnection);
			if (connection?.connected) {
				// Get schema information for autocomplete
				const schema = await DatabaseCommands.getDatabaseSchema(selectedConnection);
				monacoEditor.updateSchema(schema);
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
				monacoEditor = createMonacoEditor({
					container: editorContainer,
					value: sqlQuery,
					onChange: (newValue) => {
						sqlQuery = newValue;
					},
					onExecute: handleExecuteQuery,
					onExecuteSelection: (selectedText: string) => {
						// Simple execution of selected text
						const originalQuery = sqlQuery;
						sqlQuery = selectedText;
						handleExecuteQuery().then(() => {
							sqlQuery = originalQuery;
						});
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

<div class="flex-1 flex flex-col">
	<ResizablePaneGroup direction="vertical" class="flex-1">
		<!-- SQL Editor Pane -->
		<ResizablePane defaultSize={60} minSize={30} maxSize={80}>
			<div class="h-full p-4 pb-2">
				<Card class="h-full flex flex-col">
					<CardContent class="flex-1 p-0 min-h-0">
						<div 
							bind:this={editorContainer}
							class="w-full h-full"
						></div>
					</CardContent>
				</Card>
			</div>
		</ResizablePane>

		<ResizableHandle />

		<!-- Results & History Section Pane -->
		<ResizablePane defaultSize={40} minSize={20}>
			<div class="h-full px-4 pt-2 pb-4">
				<Card class="h-full flex flex-col overflow-hidden">
					<CardHeader class="pb-2 flex-shrink-0">
						<!-- Tab navigation -->
						<div class="flex items-center gap-1 mb-3">
							<button
								type="button"
								class="flex items-center gap-2 px-3 py-1.5 text-sm rounded-md transition-colors {activeTab === 'results' ? 'bg-primary text-primary-foreground' : 'hover:bg-muted'}"
								onclick={() => activeTab = 'results'}
							>
								<Table class="w-4 h-4" />
								Results
								{#if results.length > 0}
									<span class="text-xs opacity-75">({results.length})</span>
								{/if}
							</button>
							<button
								type="button"
								class="flex items-center gap-2 px-3 py-1.5 text-sm rounded-md transition-colors {activeTab === 'history' ? 'bg-primary text-primary-foreground' : 'hover:bg-muted'}"
								onclick={() => activeTab = 'history'}
							>
								<History class="w-4 h-4" />
								History
								{#if queryHistory.length > 0}
									<span class="text-xs opacity-75">({queryHistory.length})</span>
								{/if}
							</button>
						</div>
						
						{#if activeTab === 'results' && results.length > 0 && queryResultUI?.columns}
							<div class="flex items-center justify-between gap-4">
								<div class="flex items-center gap-2 flex-1 max-w-sm">
									<Search class="w-4 h-4 text-muted-foreground" />
									<Input
										placeholder="Search all columns..."
										bind:value={globalFilter}
										class="h-8"
									/>
								</div>
							</div>
						{/if}
					</CardHeader>
					
					<!-- Results Tab Content -->
					{#if activeTab === 'results'}
						{#if results.length > 0 && queryResultUI?.columns}
							<div class="flex-1 flex flex-col min-h-0">
								<QueryResultsTable
									data={results}
									columns={queryResultUI.columns}
									bind:table
									bind:globalFilter
								/>
							</div>
						{:else}
							<div class="flex-1 flex items-center justify-center text-muted-foreground">
								<div class="text-center">
									<div class="w-16 h-16 rounded-full bg-muted/20 flex items-center justify-center mx-auto mb-4">
										<Table class="w-8 h-8 text-muted-foreground/50" />
									</div>
									<p class="text-sm font-medium">No results to display</p>
									<p class="text-xs text-muted-foreground/70 mt-1">Run a query to see results here</p>
								</div>
							</div>
						{/if}
					{/if}
					
					<!-- History Tab Content -->
					{#if activeTab === 'history'}
						{#if queryHistory.length > 0}
							<div class="flex-1 overflow-auto">
								<div class="space-y-2 p-2">
									{#each queryHistory as historyItem}
										<button
											type="button"
											class="group w-full border rounded-lg p-3 hover:bg-muted/30 transition-colors cursor-pointer text-left"
											onclick={() => loadQueryFromHistory(historyItem.query_text)}
										>
											<div class="flex items-start justify-between gap-2 mb-2">
												<div class="flex items-center gap-2 flex-1 min-w-0">
													<div class="flex items-center gap-1">
														{#if historyItem.status === 'success'}
															<div class="w-2 h-2 rounded-full bg-green-500"></div>
														{:else}
															<div class="w-2 h-2 rounded-full bg-red-500"></div>
														{/if}
													</div>
													<span class="text-xs text-muted-foreground">
														{formatTimestamp(historyItem.executed_at)}
													</span>
													{#if historyItem.status === 'success'}
														<span class="text-xs text-muted-foreground">
															{historyItem.row_count} rows
														</span>
													{/if}
													<span class="text-xs text-muted-foreground">
														{formatDuration(historyItem.duration_ms)}
													</span>
												</div>
												<span class="opacity-0 group-hover:opacity-100 text-xs text-primary font-medium">
													Load
												</span>
											</div>
											<code class="text-xs bg-muted/50 p-2 rounded block overflow-hidden text-left">
												{historyItem.query_text.length > 200 ? historyItem.query_text.slice(0, 200) + '...' : historyItem.query_text}
											</code>
											{#if historyItem.error_message}
												<p class="text-xs text-red-600 mt-1 p-2 bg-red-50 rounded text-left">
													{historyItem.error_message}
												</p>
											{/if}
										</button>
									{/each}
								</div>
							</div>
						{:else}
							<div class="flex-1 flex items-center justify-center text-muted-foreground">
								<div class="text-center">
									<div class="w-16 h-16 rounded-full bg-muted/20 flex items-center justify-center mx-auto mb-4">
										<History class="w-8 h-8 text-muted-foreground/50" />
									</div>
									<p class="text-sm font-medium">No query history</p>
									<p class="text-xs text-muted-foreground/70 mt-1">Execute queries to see history here</p>
								</div>
							</div>
						{/if}
					{/if}
				</Card>
			</div>
		</ResizablePane>
	</ResizablePaneGroup>
</div> 