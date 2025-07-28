<script lang="ts">
	import { Table, Columns } from '@lucide/svelte';
	import type { DatabaseSchema } from '$lib/commands.svelte';

	interface Props {
		databaseSchema: DatabaseSchema | null;
		loadingSchema: boolean;
		selectedConnection: string | null;
		onTableClick?: (tableName: string, schema: string) => void;
	}

	let { databaseSchema, loadingSchema, selectedConnection, onTableClick }: Props = $props();
</script>

<div class="space-y-3">
	{#if !selectedConnection}
		<div class="text-center py-8 px-4">
			<div class="p-3 rounded-lg bg-muted/30 border border-border/50 inline-flex mb-3">
				<Table class="w-6 h-6 text-muted-foreground/50" />
			</div>
			<p class="text-xs font-medium text-muted-foreground mb-1">No connection selected</p>
			<p class="text-xs text-muted-foreground/70">Select a connection to view tables</p>
		</div>
	{:else if loadingSchema}
		<div class="text-center py-8 px-4">
			<div class="p-3 rounded-lg bg-muted/30 border border-border/50 inline-flex mb-3">
				<Table class="w-6 h-6 text-muted-foreground/50 animate-pulse" />
			</div>
			<p class="text-xs font-medium text-muted-foreground mb-1">Loading schema...</p>
		</div>
	{:else if !databaseSchema}
		<div class="text-center py-8 px-4">
			<div class="p-3 rounded-lg bg-muted/30 border border-border/50 inline-flex mb-3">
				<Table class="w-6 h-6 text-muted-foreground/50" />
			</div>
			<p class="text-xs font-medium text-muted-foreground mb-1">Schema not loaded</p>
			<p class="text-xs text-muted-foreground/70">Connect to database to view tables</p>
		</div>
	{:else if databaseSchema.tables.length === 0}
		<div class="text-center py-8 px-4">
			<div class="p-3 rounded-lg bg-muted/30 border border-border/50 inline-flex mb-3">
				<Table class="w-6 h-6 text-muted-foreground/50" />
			</div>
			<p class="text-xs font-medium text-muted-foreground mb-1">No tables found</p>
			<p class="text-xs text-muted-foreground/70">This database has no tables</p>
		</div>
	{:else}
		{#each databaseSchema.tables as table (table.name)}
			<details class="group border rounded-lg overflow-hidden">
				<summary class="flex items-center gap-3 p-3 cursor-pointer hover:bg-muted/30 transition-colors">
					<div class="flex-shrink-0">
						<Table class="w-3 h-3 text-muted-foreground" />
					</div>
					<div class="flex-1 text-left min-w-0">
						<div class="font-medium text-xs text-foreground truncate">
							{table.name}
						</div>
						<div class="text-xs text-muted-foreground/80 truncate">
							{table.schema !== 'public' ? `${table.schema}.${table.name}` : table.name} • {table.columns.length} columns
						</div>
					</div>
					{#if onTableClick}
						<button
							class="flex-shrink-0 p-1 hover:bg-primary/10 rounded text-primary/70 hover:text-primary transition-colors"
							onclick={(e) => {
								e.preventDefault();
								e.stopPropagation();
								onTableClick(table.name, table.schema);
							}}
							title="Browse table data"
						>
							<Table class="w-3 h-3" />
						</button>
					{/if}
				</summary>
				<div class="border-t bg-muted/10">
					<div class="p-2 space-y-1">
						{#each table.columns as column}
							<div class="w-full flex items-center gap-3 p-2 text-xs">
								<Columns class="w-2.5 h-2.5 text-muted-foreground/60 flex-shrink-0" />
								<div class="flex-1 min-w-0">
									<div class="font-medium text-foreground truncate">
										{column.name}
									</div>
									<div class="text-muted-foreground/70 truncate">
										{column.data_type}{column.is_nullable ? ' (nullable)' : ' (not null)'}
									</div>
								</div>
							</div>
						{/each}
					</div>
				</div>
			</details>
		{/each}
	{/if}
</div> 