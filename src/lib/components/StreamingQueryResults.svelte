<script lang="ts">
	import {
		DatabaseCommands,
		type QueryStreamData,
		type QueryStreamStart,
		type QueryStreamError
	} from '$lib/commands.svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { onDestroy } from 'svelte';
	import QueryResultsTable from './QueryResultsTable.svelte';
	import { Loader } from '@lucide/svelte';

	interface Props {
		connectionId: string;
		query: string;
		queryId?: string;
		onComplete?: (rowCount: number, duration: number) => void;
		onError?: (error: string) => void;
	}

	let { connectionId, query, queryId, onComplete, onError }: Props = $props();

	let isStreaming = $state(false);
	let streamingQueryId = $state<string | null>(null);
	let queryColumns = $state<string[]>([]);
	let queryRows = $state<any[]>([]);
	let queryRowCount = $state(0);
	let streamingStartTime = $state<number>(0);
	let streamUnlisten = $state<UnlistenFn[]>([]);
	let queryError = $state<string | null>(null);

	let table: any = $state();
	let globalFilter = $state('');

	const hasResults = $derived(queryRows.length > 0 && queryColumns.length > 0);
	const isLoading = $derived(isStreaming && queryRows.length === 0);
	const shouldShowTable = $derived(hasResults && !queryError);

	function cleanupStreamingListeners() {
		for (const unlisten of streamUnlisten) {
			try {
				unlisten();
			} catch (error) {
				console.error('Error cleaning up listener:', error);
			}
		}
		streamUnlisten = [];
		console.log('Cleanup completed');
	}

	async function setupStreamingListeners() {
		cleanupStreamingListeners();

		try {
			const streamStartUnlisten = await listen<QueryStreamStart>('query-stream-start', (event) => {
				console.log('Got query-stream-start:', event.payload);
				const { query_id, columns } = event.payload;

				if (query_id === streamingQueryId) {
					queryColumns = columns;
					queryRows = [];
					queryRowCount = 0;
					queryError = null;
				} else {
					console.log('Query ID mismatch, ignoring event');
				}
			});

			const streamDataUnlisten = await listen<QueryStreamData>('query-stream-data', (event) => {
				console.log('Got query-stream-data:', event.payload);
				const { query_id, rows: rawRows, is_complete } = event.payload;

				const rows = rawRows.trim() === '' ? [] : JSON.parse(rawRows);

				if (query_id === streamingQueryId) {
					if (is_complete) {
						console.log('Stream completed!');
						const duration = Date.now() - streamingStartTime;
						console.log('Query duration:', duration + 'ms');

						isStreaming = false;

						onComplete?.(queryRowCount, duration);
					} else {
						console.log(`Adding ${rows.length} rows to stream`);
						const newRows = rows.map((row: any[]) => {
							const rowObj: Record<string, any> = {};
							queryColumns.forEach((col, i) => {
								rowObj[col] = row[i];
							});
							return rowObj;
						});

						queryRows = queryRows.concat(newRows);
						queryRowCount += rows.length;
						console.log(`Total rows now: ${queryRowCount}`);
					}
				} else {
					console.log('Query ID mismatch, ignoring event');
				}
			});

			const streamErrorUnlisten = await listen<QueryStreamError>('query-stream-error', (event) => {
				const { query_id, error } = event.payload;

				if (query_id === streamingQueryId) {
					console.log('Stream error received:', error);
					isStreaming = false;
					let errorMessage = error;
					try {
						const parsedError = typeof error === 'string' ? JSON.parse(error) : error;
						if (parsedError && typeof parsedError === 'object' && parsedError.message) {
							errorMessage = parsedError.message;
						}
					} catch (e) {
						errorMessage = String(error);
					}
					queryError = errorMessage;
					queryRows = [];
					queryColumns = [];
					onError?.(errorMessage);
					cleanupStreamingListeners();
				} else {
					console.log('Query ID mismatch, ignoring error event');
				}
			});

			streamUnlisten = [streamStartUnlisten, streamDataUnlisten, streamErrorUnlisten];
		} catch (error) {
			console.error('❌ Failed to setup streaming listeners:', error);
		}
	}

	export async function executeQuery() {
		console.log('Starting executeQuery with:', { connectionId, query });

		cleanupStreamingListeners();

		isStreaming = true;
		streamingStartTime = Date.now();
		queryError = null;

		queryRows = [];
		queryColumns = [];
		queryRowCount = 0;

		streamingQueryId = queryId || crypto.randomUUID();

		await setupStreamingListeners();

		try {
			console.log('StartingexecuteQueryStream');
			await DatabaseCommands.executeQueryStream(connectionId, query, streamingQueryId);
			console.log('executeQueryStream finished');
		} catch (error) {
			console.error('❌ Backend error:', error);
			isStreaming = false;
			let errorMessage = String(error);
			try {
				const parsedError = typeof error === 'string' ? JSON.parse(error) : error;
				if (parsedError && typeof parsedError === 'object' && parsedError.message) {
					errorMessage = parsedError.message;
				}
			} catch (e) {
				errorMessage = String(error);
			}
			queryError = errorMessage;
			queryRows = [];
			queryColumns = [];
			onError?.(errorMessage);
			cleanupStreamingListeners();
		}
	}

	let previousConnectionId = $state('');
	let previousQuery = $state('');

	$effect(() => {
		if (
			connectionId &&
			query &&
			(connectionId !== previousConnectionId || query !== previousQuery)
		) {
			previousConnectionId = connectionId;
			previousQuery = query;

			executeQuery();
		} else {
			console.log('No execution needed - props unchanged or missing');
		}
	});

	onDestroy(() => {
		cleanupStreamingListeners();
	});
</script>

{console.log('Rendering state:', {
	queryError: !!queryError,
	isStreaming,
	rowsLength: queryRows.length,
	columnsLength: queryColumns.length,
	queryRowCount,
	hasResults,
	isLoading,
	shouldShowTable
})}

{#if queryError}
	<!-- Error state -->
	<div class="text-destructive flex flex-1 items-center justify-center">
		<div class="text-center">
			<div class="text-lg font-semibold">Query Error</div>
			<div class="text-muted-foreground mt-2">{queryError}</div>
		</div>
	</div>
{:else if shouldShowTable}
	<!-- Results available - using derived state -->
	<div class="flex h-full min-h-0 flex-1 flex-col">
		<QueryResultsTable data={queryRows} columns={queryColumns} bind:table bind:globalFilter />
		{#if isStreaming}
			<div
				class="absolute top-2 right-2 flex items-center gap-2 rounded-lg border border-blue-300 bg-blue-100 px-3 py-1 text-xs text-blue-800 shadow-sm"
			>
				<Loader class="h-3 w-3 animate-spin" />
				Streaming {queryRowCount} rows...
			</div>
		{/if}
	</div>
{:else if isLoading}
	<!-- Loading state -->
	<div class="text-muted-foreground flex flex-1 items-center justify-center">
		<div class="text-center">
			<div class="bg-muted/20 mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full">
				<Loader class="h-8 w-8 animate-spin" />
			</div>
			<div class="text-lg font-semibold">Starting query stream...</div>
			<div class="mt-1 text-sm">Query ID: {streamingQueryId}</div>
		</div>
	</div>
{:else if isStreaming}
	<!-- Loading/streaming state -->
	<div class="text-muted-foreground flex flex-1 items-center justify-center">
		<div class="text-center">
			<div class="bg-muted/20 mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full">
				<Loader class="h-8 w-8 animate-spin" />
			</div>
			<div class="text-lg font-semibold">Streaming {queryRowCount} rows...</div>
			<div class="mt-1 text-sm">Query ID: {streamingQueryId}</div>
		</div>
	</div>
{:else}
	<!-- No results -->
	<div class="text-muted-foreground flex flex-1 items-center justify-center">
		<div class="text-center">
			<div class="text-lg font-semibold">No results</div>
			<div class="mt-2">Your query returned no data</div>
		</div>
	</div>
{/if}
