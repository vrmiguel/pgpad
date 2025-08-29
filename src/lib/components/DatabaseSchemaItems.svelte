<script lang="ts">
	import { TableProperties, Columns } from '@lucide/svelte';
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
	{:else if databaseSchema.tables.length === 0}
		<div class="px-4 py-8 text-center">
			<div class="bg-muted/30 border-border/50 mb-3 inline-flex rounded-lg border p-3">
				<TableProperties class="text-muted-foreground/50 h-6 w-6" />
			</div>
			<p class="text-muted-foreground mb-1 text-xs font-medium">No tables found</p>
			<p class="text-muted-foreground/70 text-xs">This database has no tables</p>
		</div>
	{:else}
		{#each databaseSchema.tables as table (table.name)}
			<details class="group overflow-hidden rounded-lg border">
				<summary
					class="hover:bg-muted/30 flex cursor-pointer items-center gap-3 p-3 transition-colors"
				>
					<div class="flex-shrink-0">
						<TableProperties class="text-muted-foreground h-3 w-3" />
					</div>
					<div class="min-w-0 flex-1 text-left">
						<div class="text-foreground truncate text-xs font-medium">
							{table.name}
						</div>
						<div class="text-muted-foreground/80 truncate text-xs">
							{table.schema && table.schema !== 'public'
								? `${table.schema}.${table.name}`
								: table.name} • {table.columns.length} columns
						</div>
					</div>
					{#if onTableClick}
						<button
							class="hover:bg-primary/10 text-primary/70 hover:text-primary flex-shrink-0 rounded p-1 transition-colors"
							onclick={(e) => {
								e.preventDefault();
								e.stopPropagation();
								onTableClick(table.name, table.schema);
							}}
							title="Browse table data"
						>
							<TableProperties class="h-3 w-3" />
						</button>
					{/if}
				</summary>
				<div class="bg-muted/10 border-t">
					<div class="space-y-1 p-2">
						{#each table.columns as column}
							<div class="flex w-full items-center gap-3 p-2 text-xs">
								<Columns class="text-muted-foreground/60 h-2.5 w-2.5 flex-shrink-0" />
								<div class="min-w-0 flex-1">
									<div class="text-foreground truncate font-medium">
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
