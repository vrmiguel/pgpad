<script lang="ts">
	import { Database, Circle, AlertTriangle, Loader2 } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';

	interface Connection {
		id: string;
		name: string;
		connection_string: string;
		connected: boolean;
	}

	interface Props {
		connections: Connection[];
		selectedConnection: string | null;
		establishingConnections: Set<string>;
		onSelect: (connectionId: string) => void;
		onConnect?: (connectionId: string) => void;
	}

	let { connections, selectedConnection, establishingConnections, onSelect, onConnect }: Props = $props();
</script>

<div class="flex-1 overflow-y-auto bg-gradient-to-b from-transparent to-muted/10">
	<div class="p-4">
		<h2 class="text-sm font-semibold text-muted-foreground/80 mb-4 px-3 uppercase tracking-wider">Connections</h2>
		
		{#if connections.length === 0}
			<div class="text-center py-12 px-4">
				<div class="p-4 rounded-xl bg-muted/30 border border-border/50 inline-flex mb-4">
					<Database class="w-8 h-8 text-muted-foreground/50" />
				</div>
				<p class="text-sm font-medium text-muted-foreground mb-2">No connections yet</p>
				<p class="text-xs text-muted-foreground/70">Add your first connection to get started</p>
			</div>
		{:else}
			<div class="space-y-2">
				{#each connections as connection (connection.id)}
					<Button
						variant={selectedConnection === connection.id ? "secondary" : "ghost"}
						class="w-full justify-start p-4 h-auto shadow-sm hover:shadow-md transition-all duration-200 {selectedConnection === connection.id ? 'shadow-md bg-primary/10 border border-primary/20' : ''}"
						onclick={() => onSelect(connection.id)}
						ondblclick={() => onConnect?.(connection.id)}
					>
						<div class="flex items-start gap-4 w-full">
							<div class="flex-shrink-0 mt-1">
								{#if connection.connected}
									<div class="w-3 h-3 rounded-full bg-success border-2 border-success-light shadow-sm"></div>
								{:else if establishingConnections.has(connection.id)}
									<Loader2 class="w-3 h-3 text-primary animate-spin" />
								{:else}
									<div class="w-3 h-3 rounded-full bg-muted-foreground/40 border-2 border-muted-foreground/20"></div>
								{/if}
							</div>
							
							<div class="flex-1 text-left min-w-0">
								<div class="font-medium text-sm text-foreground truncate mb-1">
									{connection.name}
								</div>
								<div class="text-xs text-muted-foreground/80 truncate mb-1">
									{connection.connection_string.replace(/^postgresql?:\/\/[^@]*@/, '').replace(/\/[^?]*/, '')}
								</div>
								<div class="text-xs text-muted-foreground/60 truncate mb-2">
									{connection.connection_string.split('/').pop()?.split('?')[0] || 'database'}
								</div>
								{#if !connection.connected}
									<div class="flex items-center gap-2 mt-2">
										{#if establishingConnections.has(connection.id)}
											<div class="flex items-center gap-2 px-2 py-1 rounded-md bg-primary/10 border border-primary/20">
												<Loader2 class="w-3 h-3 text-primary animate-spin" />
												<span class="text-xs font-medium text-primary">Connecting...</span>
											</div>
										{:else}
											<div class="flex items-center gap-2 px-2 py-1 rounded-md bg-muted/30 border border-border">
												<AlertTriangle class="w-3 h-3 text-muted-foreground" />
												<span class="text-xs font-medium text-muted-foreground">Disconnected</span>
											</div>
										{/if}
									</div>
								{/if}
							</div>
						</div>
					</Button>
				{/each}
			</div>
		{/if}
	</div>
</div> 