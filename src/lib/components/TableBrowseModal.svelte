<script lang="ts">
	import { Search, Table, Loader, X } from '@lucide/svelte';
	import { Input } from '$lib/components/ui/input';
	import { Card, CardHeader, CardTitle, CardContent } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import QueryResultsTable from './QueryResultsTable.svelte';
	import { DatabaseCommands, type QueryResult } from '$lib/commands.svelte';

	interface Props {
		isOpen: boolean;
		tableName: string;
		schema: string;
		connectionId: string;
		onClose: () => void;
	}

	let { isOpen, tableName, schema, connectionId, onClose }: Props = $props();

	let queryResult = $state<QueryResult | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let table: any = $state();
	let globalFilter = $state('');

	// Convert query result to format expected by QueryResultsTable  
	const results = $derived.by(() => {
		if (!queryResult) return [];
		
		return queryResult.rows.map(row => {
			const rowObj: Record<string, any> = {};
			queryResult!.columns.forEach((col, i) => {
				rowObj[col] = row[i];
			});
			return rowObj;
		});
	});

	const columns = $derived.by(() => queryResult?.columns || []);

	// Load table data when modal opens
	$effect(() => {
		if (isOpen && tableName && connectionId) {
			loadTableData();
		}
	});

	async function loadTableData() {
		loading = true;
		error = null;
		queryResult = null;

		try {
			const query = schema === 'public' 
				? `SELECT * FROM ${tableName} LIMIT 1000`
				: `SELECT * FROM "${schema}"."${tableName}" LIMIT 1000`;
			
			queryResult = await DatabaseCommands.executeQuery(connectionId, query);
		} catch (err) {
			error = String(err);
		} finally {
			loading = false;
		}
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onClose();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onClose();
		}
	}
</script>

<svelte:window on:keydown={handleKeydown} />

{#if isOpen}
	<div 
		class="fixed inset-0 bg-background/80 backdrop-blur-sm flex items-center justify-center z-50"
		role="dialog"
		aria-modal="true"
		tabindex="-1"
		onclick={handleBackdropClick}
		onkeydown={handleKeydown}
	>
		<Card class="w-[90vw] h-[90vh] flex flex-col overflow-hidden shadow-xl">
			<CardHeader class="pb-2 flex-shrink-0">
				<div class="flex items-center justify-between">
					<CardTitle class="flex items-center gap-2 text-lg">
						<Table class="w-5 h-5" />
						{schema !== 'public' ? `${schema}.${tableName}` : tableName}
					</CardTitle>
					<Button variant="ghost" size="sm" onclick={onClose}>
						<X class="w-4 h-4" />
					</Button>
				</div>
				
				{#if loading}
					<div class="flex items-center gap-2 text-sm text-muted-foreground">
						<Loader class="w-4 h-4 animate-spin" />
						Loading table data...
					</div>
				{:else if error}
					<div class="text-sm text-destructive">
						Error: {error}
					</div>
				{:else if results.length > 0 && columns.length > 0}
					<div class="flex items-center justify-between gap-4">
						<div class="flex items-center gap-2 flex-1 max-w-sm">
							<Search class="w-4 h-4 text-muted-foreground" />
							<Input
								placeholder="Search all columns..."
								bind:value={globalFilter}
								class="h-8"
							/>
						</div>
						<div class="text-sm text-muted-foreground">
							{results.length} row{results.length === 1 ? '' : 's'}
							{#if queryResult}
								• {queryResult.duration_ms}ms
							{/if}
						</div>
					</div>
				{/if}
			</CardHeader>
			
			<CardContent class="flex-1 flex flex-col min-h-0">
				{#if loading}
					<div class="flex-1 flex items-center justify-center text-muted-foreground">
						<div class="text-center">
							<div class="w-16 h-16 rounded-full bg-muted/20 flex items-center justify-center mx-auto mb-4">
								<Loader class="w-8 h-8 text-muted-foreground/50 animate-spin" />
							</div>
							<p class="text-sm font-medium">Loading table data</p>
							<p class="text-xs text-muted-foreground/70 mt-1">Executing SELECT * FROM {tableName}</p>
						</div>
					</div>
				{:else if error}
					<div class="flex-1 flex items-center justify-center text-muted-foreground">
						<div class="text-center">
							<div class="w-16 h-16 rounded-full bg-destructive/10 flex items-center justify-center mx-auto mb-4">
								<Table class="w-8 h-8 text-destructive/50" />
							</div>
							<p class="text-sm font-medium text-destructive">Failed to load table</p>
							<p class="text-xs text-muted-foreground/70 mt-1">{error}</p>
						</div>
					</div>
				{:else if results.length > 0 && columns.length > 0}
					<div class="flex-1 flex flex-col min-h-0">
						<QueryResultsTable
							data={results}
							{columns}
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
							<p class="text-sm font-medium">Table is empty</p>
							<p class="text-xs text-muted-foreground/70 mt-1">No data found in {tableName}</p>
						</div>
					</div>
				{/if}
			</CardContent>
		</Card>
	</div>
{/if} 