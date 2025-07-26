<script lang="ts">
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { ResizablePaneGroup, ResizablePane, ResizableHandle } from '$lib/components/ui/resizable';
	import { Table, FileText, Clock, Search } from '@lucide/svelte';
	import { DatabaseCommands, type ConnectionInfo, type QueryResult, type QueryHistoryEntry, type DatabaseSchema } from '$lib/commands.svelte';
	import { createMonacoEditor, type CreateMonacoEditorOptions } from '$lib/monaco';
	import { onMount, onDestroy } from 'svelte';
	import QueryResultsTable from './QueryResultsTable.svelte';

	interface Props {
		selectedConnection: string | null;
		connections: ConnectionInfo[];
	}

	let { selectedConnection, connections }: Props = $props();
	
	let sqlQuery = $state(`-- Welcome to PgPad!
-- Keyboard shortcuts:
--   Ctrl+Enter: Run selected text (or current line if nothing selected)
--   Ctrl+R: Run entire script

SELECT 
    table_name,
    column_name,
    data_type
FROM information_schema.columns
WHERE table_schema = 'public'
ORDER BY table_name, ordinal_position;`);

	let queryResult = $state<QueryResult | null>(null);
	let isExecuting = $state(false);
	let queryHistory: QueryHistoryEntry[] = $state([]);
	let databaseSchema = $state<DatabaseSchema | null>(null);

	let table: any = $state();
	let globalFilter = $state('');

	// Load query history from storage
	async function loadQueryHistory() {
		if (!selectedConnection) {
			queryHistory = [];
			return;
		}

		try {
			const history = await DatabaseCommands.getQueryHistory(selectedConnection, 50);
			queryHistory = history;
		} catch (error) {
			console.error('Failed to load query history:', error);
			queryHistory = [];
		}
	}

	async function loadDatabaseSchema() {
		if (!selectedConnection) {
			databaseSchema = null;
			return;
		}

		const connection = connections.find(c => c.id === selectedConnection);
		if (!connection?.connected) {
			databaseSchema = null;
			return;
		}

		try {
			const schema = await DatabaseCommands.getDatabaseSchema(selectedConnection);
			databaseSchema = schema;
			console.log('Loaded database schema:', schema);
			
			if (monacoEditor) {
				monacoEditor.updateSchema(schema);
			}
		} catch (error) {
			console.error('Failed to load database schema:', error);
			databaseSchema = null;
		}
	}

	function formatTimestamp(timestamp: number): string {
		return new Date(timestamp * 1000).toLocaleString();
	}

	function formatDuration(durationMs: number | null): string {
		if (durationMs === null) return '0ms';
		return `${durationMs}ms`;
	}

	// Monaco stuff **
	let editorContainer: HTMLElement;
	let monacoEditor: ReturnType<typeof createMonacoEditor> | null = null;
    //              **

	export async function handleExecuteQuery() {
		if (!selectedConnection || !sqlQuery.trim()) return;
		
		if (!isConnected()) {
			console.warn('Cannot execute query: No active database connection');
			
			try {
				await DatabaseCommands.saveQueryToHistory(
					selectedConnection,
					sqlQuery.trim(),
					null,
					'error',
					0,
					'No active database connection. Please connect to a database first.'
				);
				await loadQueryHistory();
			} catch (error) {
				console.error('Failed to save query to history:', error);
			}
			return;
		}

		await executeQuery(sqlQuery.trim());
	}

	async function handleExecuteSelection(selectedText: string) {
		if (!selectedConnection || !selectedText) return;
		
		if (!isConnected()) {
			console.warn('Cannot execute query: No active database connection');
			return;
		}

		await executeQuery(selectedText);
	}

	async function executeQuery(queryText: string) {
		if (!selectedConnection) return;
		
		isExecuting = true;
		const start = Date.now();

		try {
			const result = await DatabaseCommands.executeQuery(selectedConnection, queryText);
			queryResult = result;

			try {
				await DatabaseCommands.saveQueryToHistory(
					selectedConnection,
					queryText,
					result.duration_ms,
					'success',
					result.row_count,
					null
				);
				await loadQueryHistory();
			} catch (error) {
				console.error('Failed to save query to history:', error);
			}
		} catch (error) {
			console.error('Query execution failed:', error);
			
			try {
				await DatabaseCommands.saveQueryToHistory(
					selectedConnection,
					queryText,
					Date.now() - start,
					'error',
					0,
					String(error)
				);
				await loadQueryHistory();
			} catch (historyError) {
				console.error('Failed to save query to history:', historyError);
			}
		} finally {
			isExecuting = false;
		}
	}

	const isConnected = $derived(() => {
		if (!selectedConnection) return false;
		const connection = connections.find(c => c.id === selectedConnection);
		return connection?.connected || false;
	});

	onMount(() => {
		// waits a bit to ensure DOM is ready
		setTimeout(() => {
			if (editorContainer) {
				monacoEditor = createMonacoEditor({
					container: editorContainer,
					value: sqlQuery,
					onChange: (value) => {
						sqlQuery = value;
					},
					onExecute: handleExecuteQuery,
					onExecuteSelection: handleExecuteSelection,
					disabled: false,
					theme: 'light', // TODO
					schema: databaseSchema
				});
			}
		}, 0);
	});

	onDestroy(() => {
		monacoEditor?.dispose();
	});

	$effect(() => {
		if (monacoEditor) {
			monacoEditor.updateValue(sqlQuery);
		}
	});

	// Load query history on connection changes
	$effect(() => {
		loadQueryHistory();
	});

	// Load database schema on connection changes
	$effect(() => {
		loadDatabaseSchema();
	});

	const results = $derived(queryResult?.rows.map(row => {
		const rowObj: Record<string, any> = {};
		queryResult?.columns.forEach((col, i) => {
			rowObj[col] = row[i];
		});
		return rowObj;
	}) || []);
</script>

<div class="flex-1 flex flex-col">
	<ResizablePaneGroup direction="vertical" class="flex-1">
		<!-- SQL Editor Pane -->
		<ResizablePane defaultSize={60} minSize={30} maxSize={80}>
			<div class="h-full p-4 pb-2">
				<Card class="h-full flex flex-col">
					<CardHeader class="flex-shrink-0">
						<CardTitle class="flex items-center gap-2">
							<FileText class="w-4 h-4" />
							SQL Editor
						</CardTitle>
					</CardHeader>
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

		<!-- Results Section Pane -->
		<ResizablePane defaultSize={40} minSize={20}>
			<div class="h-full px-4 pt-2 pb-4">
				<ResizablePaneGroup direction="horizontal" class="h-full">
					<!-- Query Results Pane -->
					<ResizablePane defaultSize={70} minSize={40}>
						<Card class="h-full flex flex-col overflow-hidden mr-2">
							<CardHeader class="pb-2 flex-shrink-0">
								<CardTitle class="flex items-center gap-2 mb-3">
									<Table class="w-4 h-4" />
									Results
									{#if results.length > 0}
										<span class="text-sm font-normal text-muted-foreground">({results.length} rows)</span>
									{/if}
								</CardTitle>
								
								{#if results.length > 0 && queryResult?.columns}
									<!-- Search integrated into header -->
									<div class="flex items-center justify-between gap-4">
										<div class="flex items-center gap-2 flex-1 max-w-sm">
											<Search class="w-4 h-4 text-muted-foreground" />
											<Input
												placeholder="Search all columns..."
												bind:value={globalFilter}
												class="h-8"
											/>
										</div>
										
										<div class="flex items-center gap-2 text-sm text-muted-foreground">
											<span>
												{table?.getFilteredRowModel().rows.length || 0} of {table?.getCoreRowModel().rows.length || 0} row(s)
											</span>
										</div>
									</div>
								{/if}
							</CardHeader>
							
							{#if results.length > 0 && queryResult?.columns}
								<!-- Table fills remaining space -->
								<div class="flex-1 flex flex-col min-h-0">
									<QueryResultsTable
										data={results}
										columns={queryResult.columns}
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
						</Card>
					</ResizablePane>

					<ResizableHandle />

					<!-- Query History Pane -->
					<ResizablePane defaultSize={30} minSize={25} maxSize={50}>
						<Card class="h-full flex flex-col ml-2">
							<CardHeader class="flex-shrink-0">
								<CardTitle class="flex items-center gap-2">
									<Clock class="w-4 h-4" />
									Query History
								</CardTitle>
							</CardHeader>
							<CardContent class="p-0 flex-1 min-h-0">
								<div class="overflow-auto h-full">
									{#if queryHistory.length > 0}
										<div class="space-y-2 p-3">
											{#each queryHistory as query}
												<div class="border rounded-lg p-3 hover:bg-gray-50 cursor-pointer">
													<div class="font-mono text-xs text-gray-600 mb-1 truncate">
														{query.query_text}
													</div>
													<div class="flex items-center justify-between text-xs text-gray-500">
														<span>{formatTimestamp(query.executed_at)}</span>
														<div class="flex items-center gap-2">
															<span class="px-1.5 py-0.5 rounded text-xs {query.status === 'success' ? 'bg-green-100 text-green-700' : 'bg-red-100 text-red-700'}">
																{query.status}
															</span>
															<span>{formatDuration(query.duration_ms)}</span>
														</div>
													</div>
													{#if query.status === 'success'}
														<div class="text-xs text-gray-400 mt-1">
															{query.row_count} row{query.row_count !== 1 ? 's' : ''}
														</div>
													{:else if query.error_message}
														<div class="text-xs text-red-500 mt-1 truncate">
															{query.error_message}
														</div>
													{/if}
												</div>
											{/each}
										</div>
									{:else}
										<div class="flex items-center justify-center h-full text-gray-500">
											<div class="text-center">
												<Clock class="w-8 h-8 mx-auto mb-2 opacity-50" />
												<p class="text-sm">No queries yet</p>
												<p class="text-xs text-gray-400 mt-1">Your query history will appear here</p>
											</div>
										</div>
									{/if}
								</div>
							</CardContent>
						</Card>
					</ResizablePane>
				</ResizablePaneGroup>
			</div>
		</ResizablePane>
	</ResizablePaneGroup>
</div> 