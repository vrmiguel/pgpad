<script lang="ts">
	import { History } from '@lucide/svelte';

	import type { QueryHistoryEntry } from '$lib/commands.svelte';

	interface Props {
		queryHistory: QueryHistoryEntry[];
		onLoadFromHistory?: (historyQuery: string) => void;
	}

	const { queryHistory, onLoadFromHistory }: Props = $props();
</script>

{#if queryHistory.length > 0}
	<div class="scrollable-container h-full space-y-2 overflow-y-auto">
		{#each queryHistory as historyItem (historyItem.id)}
			<button
				type="button"
				class="group hover:bg-muted/30 w-full cursor-pointer rounded-sm border p-2 text-left transition-colors"
				onclick={() => onLoadFromHistory?.(historyItem.query_text)}
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
							{new Date(historyItem.executed_at * 1000).toLocaleDateString()}
						</span>
						{#if historyItem.status === 'success'}
							<span class="text-muted-foreground text-xs">
								{historyItem.row_count} rows
							</span>
						{/if}
						<span class="text-muted-foreground text-xs">
							{historyItem.duration_ms}ms
						</span>
					</div>
					<span class="text-primary text-xs font-medium opacity-0 group-hover:opacity-100">
						Load
					</span>
				</div>
				<code class="bg-muted/50 block overflow-hidden rounded p-2 text-left text-xs">
					{historyItem.query_text.length > 100
						? historyItem.query_text.slice(0, 100) + '...'
						: historyItem.query_text}
				</code>
				{#if historyItem.error_message}
					<p class="bg-error/50 mt-1 rounded p-2 text-left text-xs">
						{historyItem.error_message}
					</p>
				{/if}
			</button>
		{/each}
	</div>
{:else}
	<div class="text-muted-foreground flex flex-1 items-center justify-center py-8">
		<div class="text-center">
			<div class="bg-muted/30 border-border/50 mb-3 inline-flex rounded-lg border p-3">
				<History class="text-muted-foreground/50 h-6 w-6" />
			</div>
			<p class="text-muted-foreground mb-1 text-xs font-medium">No query history</p>
			<p class="text-muted-foreground/70 text-xs">Execute queries to see history here</p>
		</div>
	</div>
{/if}
