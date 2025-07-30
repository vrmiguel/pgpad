<script lang="ts">
	import { ResizablePaneGroup, ResizablePane, ResizableHandle } from '$lib/components/ui/resizable';
	import { Card, CardHeader, CardTitle, CardContent } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import StreamingQueryResults from './StreamingQueryResults.svelte';
	import {
		DatabaseCommands,
		type ConnectionInfo,
		type Script,
		type QueryHistoryEntry
	} from '$lib/commands.svelte';
	import { createEditor, type CreateEditorOptions } from '$lib/codemirror';
	import { onMount } from 'svelte';
	import { ChevronDown, ChevronRight, Play, Loader, Table, Clock, History } from '@lucide/svelte';

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
	let activeTab = $state<'results' | 'history'>('results');

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
			queryHistory = await DatabaseCommands.getQueryHistory(selectedConnection, 50);
		} catch (error) {
			console.error('Failed to load query history:', error);
			queryHistory = [];
		}
	}

	function handleQueryComplete(rowCount: number, duration: number) {
		// Don't set isExecuting = false here!
		// Let the StreamingQueryResults component stay mounted to show results

		// Save successful query to history
		if (selectedConnection) {
			DatabaseCommands.saveQueryToHistory(
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
		// Don't set isExecuting = false here either
		// Let the StreamingQueryResults component handle error display

		if (selectedConnection) {
			DatabaseCommands.saveQueryToHistory(selectedConnection, currentQuery, 0, 'error', 0, error);
			loadQueryHistory();
		}
	}

	export function handleExecuteQuery(queryToExecute?: string) {
		const query = queryToExecute || sqlQuery;
		if (!selectedConnection || !query.trim()) return;

		if (!isConnected) {
			console.warn('Cannot execute query: No active database connection');
			return;
		}

		currentQuery = query.trim();
		// Remove isExecuting = true since we're not using it for component mounting anymore
		activeTab = 'results';
	}

	export function handleExecuteQueryStream(queryToExecute?: string) {
		return handleExecuteQuery(queryToExecute);
	}

	function loadQueryFromHistory(historyQuery: string) {
		if (onLoadFromHistory) {
			// Create new tab with history content
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

	// Load database schema for editor completion
	async function loadDatabaseSchema() {
		if (!selectedConnection || !sqlEditor) return;

		try {
			const connection = connections.find((c) => c.id === selectedConnection);
			if (connection?.connected) {
				// Get schema information for autocomplete
				const schema = await DatabaseCommands.getDatabaseSchema(selectedConnection);
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
			<div class="h-full p-1 pb-0">
				<Card class="flex h-full flex-col py-0 gap-0">
					<CardContent class="min-h-0 flex-1 p-0">
						<div bind:this={editorContainer} class="h-full w-full overflow-hidden rounded-md"></div>
					</CardContent>
				</Card>
			</div>
		</ResizablePane>

		<ResizableHandle />

		<!-- Results & History Section Pane -->
		<ResizablePane defaultSize={40} minSize={20}>
			<div class="h-full px-1 pt-0 pb-1">
				<Card class="flex h-full flex-col overflow-hidden py-2 gap-1">
					<CardHeader class="flex-shrink-0 pb-1">
						<!-- Tab navigation -->
						<div class="mb-1 flex items-center gap-1">
							<button
								type="button"
								class="flex items-center gap-2 rounded px-2 py-1 text-xs transition-colors {activeTab ===
								'results'
									? 'bg-primary text-primary-foreground'
									: 'hover:bg-muted'}"
								onclick={() => (activeTab = 'results')}
							>
								<Table class="h-3 w-3" />
								Results
							</button>
							<button
								type="button"
								class="flex items-center gap-2 rounded px-2 py-1 text-xs transition-colors {activeTab ===
								'history'
									? 'bg-primary text-primary-foreground'
									: 'hover:bg-muted'}"
								onclick={() => (activeTab = 'history')}
							>
								<History class="h-3 w-3" />
								History
								{#if queryHistory.length > 0}
									<span class="text-xs opacity-75">({queryHistory.length})</span>
								{/if}
							</button>
						</div>
					</CardHeader>

					{#if activeTab === 'results'}
						<CardContent class="flex min-h-0 flex-1 flex-col p-0">
							{#if currentQuery && selectedConnection}
								<StreamingQueryResults
									connectionId={selectedConnection}
									query={currentQuery}
									onComplete={handleQueryComplete}
									onError={handleQueryError}
								/>
							{:else}
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
							{/if}
						</CardContent>
					{:else if activeTab === 'history'}
						<CardContent class="flex min-h-0 flex-1 flex-col p-0">
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
					{/if}
				</Card>
			</div>
		</ResizablePane>
	</ResizablePaneGroup>
</div>
