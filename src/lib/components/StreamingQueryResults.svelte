<script lang="ts">
	import { Commands, type QueryStreamEvent, type PgRow } from '$lib/commands.svelte';
	import { onDestroy } from 'svelte';
	import QueryResultsTable from './QueryResultsTable.svelte';
	import { Loader } from '@lucide/svelte';

	interface Props {
		connectionId: string;
		query: string;
		onComplete?: (rowCount: number, duration: number) => void;
		onError?: (error: string) => void;
	}

	let { connectionId, query, onComplete, onError }: Props = $props();

	let isStreaming = $state(false);
	let queryColumns = $state<string[]>([]);
	let queryRows = $state<PgRow[]>([]);
	let queryRowCount = $state(0);
	let streamingStartTime = $state<number>(0);
	let queryError = $state<string | null>(null);

	let table: any = $state();
	let globalFilter = $state('');

	const hasResults = $derived(queryRows.length > 0 && queryColumns.length > 0);
	const isLoading = $derived(isStreaming && queryRows.length === 0);
	const shouldShowTable = $derived(hasResults && !queryError);

	function handleQueryStreamEvent(event: QueryStreamEvent) {
		console.log('Got query stream event:', event);

		switch (event.event) {
			case 'start':
				console.log('Query stream started with columns:', event.data.columns);
				queryColumns = event.data.columns;
				queryRows = [];
				queryRowCount = 0;
				queryError = null;
				break;

			case 'batch':
				console.log('Got batch data');
				const rows = event.data.rows || [];

				console.log(`Adding ${rows.length} rows to stream`);

				queryRows = queryRows.concat(rows);
				queryRowCount += rows.length;
				console.log(`Total rows now: ${queryRowCount}`);
				break;

			case 'finish':
				console.log('Stream completed!');
				const duration = Date.now() - streamingStartTime;
				console.log('Query duration:', duration + 'ms');

				isStreaming = false;
				onComplete?.(queryRowCount, duration);
				break;

			case 'error':
				console.log('Stream error received:', event.data.error);
				isStreaming = false;
				let errorMessage = event.data.error;
				try {
					const parsedError =
						typeof errorMessage === 'string' ? JSON.parse(errorMessage) : errorMessage;
					if (parsedError && typeof parsedError === 'object' && parsedError.message) {
						errorMessage = parsedError.message;
					}
				} catch (e) {
					errorMessage = String(errorMessage);
				}
				queryError = errorMessage;
				queryRows = [];
				queryColumns = [];
				onError?.(errorMessage);
				break;
		}
	}

	export async function executeQuery() {
		console.log('Starting executeQuery with:', { connectionId, query });

		isStreaming = true;
		streamingStartTime = Date.now();
		queryError = null;

		queryRows = [];
		queryColumns = [];
		queryRowCount = 0;

		try {
			console.log('Starting executeQueryStream');
			await Commands.executeQueryStream(connectionId, query, handleQueryStreamEvent);
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
	<!-- Results available -->
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
