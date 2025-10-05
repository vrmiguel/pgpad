<script lang="ts">
	import { TableProperties, Search, ChevronRightIcon } from '@lucide/svelte';
	import type { DatabaseSchema } from '$lib/commands.svelte';

	interface Props {
		databaseSchema: DatabaseSchema | null;
		loadingSchema: boolean;
		selectedConnection: string | null;
		onTableClick?: (tableName: string, schema: string) => void;
	}

	let { databaseSchema, loadingSchema, selectedConnection, onTableClick }: Props = $props();

	const sortedTables = $derived(
		databaseSchema?.tables?.toSorted((a, b) => a.name.localeCompare(b.name)) || []
	);
</script>

<div class="scrollable-container h-full space-y-3 overflow-y-auto">
	{#if !selectedConnection}
		<div class="px-4 py-8 text-center">
			<div class="bg-muted/30 border-border/50 mb-3 inline-flex rounded-lg border p-3">
				<TableProperties class="text-muted-foreground/50 h-6 w-6" />
			</div>
			<p class="text-muted-foreground mb-1 text-xs font-medium">No connection selected</p>
			<p class="text-muted-foreground/70 text-xs">Select a connection to view tables</p>
		</div>
	{:else if loadingSchema}
		<div class="px-4 py-8 text-center">
			<div class="bg-muted/30 border-border/50 mb-3 inline-flex rounded-lg border p-3">
				<TableProperties class="text-muted-foreground/50 h-6 w-6 animate-pulse" />
			</div>
			<p class="text-muted-foreground mb-1 text-xs font-medium">Loading schema...</p>
		</div>
	{:else if !databaseSchema}
		<div class="px-4 py-8 text-center">
			<div class="bg-muted/30 border-border/50 mb-3 inline-flex rounded-lg border p-3">
				<TableProperties class="text-muted-foreground/50 h-6 w-6" />
			</div>
			<p class="text-muted-foreground mb-1 text-xs font-medium">Schema not loaded</p>
			<p class="text-muted-foreground/70 text-xs">Connect to database to view tables</p>
		</div>
	{:else if databaseSchema?.tables?.length === 0}
		<div class="px-4 py-8 text-center">
			<div class="bg-muted/30 border-border/50 mb-3 inline-flex rounded-lg border p-3">
				<TableProperties class="text-muted-foreground/50 h-6 w-6" />
			</div>
			<p class="text-muted-foreground mb-1 text-xs font-medium">No tables found</p>
			<p class="text-muted-foreground/70 text-xs">This database has no tables</p>
		</div>
	{:else}
		{#each sortedTables as table (table.name)}
			<details class="group">
				<summary
					class="relative flex list-none items-center gap-3 rounded-none p-1 transition-all duration-200 hover:bg-black/3 dark:hover:bg-white/3"
				>
					<ChevronRightIcon class="text-muted-foreground/80 h-4 w-4 group-open:rotate-90"
					></ChevronRightIcon>
					<div class="min-w-0 flex-1 text-left">
						<div class="text-foreground truncate text-sm font-medium">
							{table.name}
						</div>
						<div class="text-muted-foreground/60 truncate text-xs">
							{table.schema && table.schema !== 'public' ? `${table.schema} • ` : ''}{table.columns
								.length} columns
						</div>
					</div>
					{#if onTableClick}
						<button
							class="text-muted-foreground/70 flex-shrink-0 cursor-pointer rounded-md p-1.5 opacity-0 transition-all duration-200 group-hover:opacity-100 hover:bg-blue-100 hover:text-blue-600 dark:hover:bg-blue-900/30 dark:hover:text-blue-400"
							onclick={(e) => {
								e.preventDefault();
								e.stopPropagation();
								onTableClick(table.name, table.schema);
							}}
							title="Browse table data"
						>
							<Search class="h-3.5 w-3.5" />
						</button>
					{/if}
					<div class="bg-border/40 absolute right-0 bottom-0 left-0 h-px"></div>
				</summary>
				<div class="relative ml-5 space-y-0.5">
					{#each table.columns as column (column.name)}
						<div
							class="flex items-center gap-2 rounded-none px-2 py-1.5 text-xs transition-colors duration-200 hover:bg-black/2 dark:hover:bg-white/2"
						>
							<div class="flex-shrink-0">
								<div class="bg-muted-foreground/30 h-1 w-1 rounded-full"></div>
							</div>
							<div class="min-w-0 flex-1">
								<span class="text-foreground font-medium">{column.name}</span>
								<span class="text-muted-foreground/60 ml-2 text-xs">
									{column.data_type}{column.is_nullable ? '' : ' • not null'}
								</span>
							</div>
						</div>
					{/each}
				</div>
			</details>
		{/each}
	{/if}
</div>
