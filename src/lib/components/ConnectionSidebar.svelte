<script lang="ts">
	import { Database, AlertTriangle, Loader2 } from '@lucide/svelte';
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

	let { connections, selectedConnection, establishingConnections, onSelect, onConnect }: Props =
		$props();
</script>

<div class="to-muted/10 flex-1 overflow-y-auto bg-gradient-to-b from-transparent">
	<div class="p-4">
		<h2 class="text-muted-foreground/80 mb-4 px-3 text-sm font-semibold tracking-wider uppercase">
			Connections
		</h2>

		{#if connections.length === 0}
			<div class="px-4 py-12 text-center">
				<div class="bg-muted/30 border-border/50 mb-4 inline-flex rounded-xl border p-4">
					<Database class="text-muted-foreground/50 h-8 w-8" />
				</div>
				<p class="text-muted-foreground mb-2 text-sm font-medium">No connections yet</p>
				<p class="text-muted-foreground/70 text-xs">Add your first connection to get started</p>
			</div>
		{:else}
			<div class="space-y-2">
				{#each connections as connection (connection.id)}
					<Button
						variant={selectedConnection === connection.id ? 'secondary' : 'ghost'}
						class="h-auto w-full justify-start p-4 shadow-sm transition-all duration-200 hover:shadow-md {selectedConnection ===
						connection.id
							? 'bg-primary/10 border-primary/20 border shadow-md'
							: ''}"
						onclick={() => onSelect(connection.id)}
						ondblclick={() => onConnect?.(connection.id)}
					>
						<div class="flex w-full items-start gap-4">
							<div class="mt-1 flex-shrink-0">
								{#if connection.connected}
									<div
										class="bg-success border-success-light h-3 w-3 rounded-full border-2 shadow-sm"
									></div>
								{:else if establishingConnections.has(connection.id)}
									<Loader2 class="text-primary h-3 w-3 animate-spin" />
								{:else}
									<div
										class="bg-muted-foreground/40 border-muted-foreground/20 h-3 w-3 rounded-full border-2"
									></div>
								{/if}
							</div>

							<div class="min-w-0 flex-1 text-left">
								<div class="text-foreground mb-1 truncate text-sm font-medium">
									{connection.name}
								</div>
								<div class="text-muted-foreground/80 mb-1 truncate text-xs">
									{connection.connection_string
										.replace(/^postgresql?:\/\/[^@]*@/, '')
										.replace(/\/[^?]*/, '')}
								</div>
								<div class="text-muted-foreground/60 mb-2 truncate text-xs">
									{connection.connection_string.split('/').pop()?.split('?')[0] || 'database'}
								</div>
								{#if !connection.connected}
									<div class="mt-2 flex items-center gap-2">
										{#if establishingConnections.has(connection.id)}
											<div
												class="bg-primary/10 border-primary/20 flex items-center gap-2 rounded-md border px-2 py-1"
											>
												<Loader2 class="text-primary h-3 w-3 animate-spin" />
												<span class="text-primary text-xs font-medium">Connecting...</span>
											</div>
										{:else}
											<div
												class="bg-muted/30 border-border flex items-center gap-2 rounded-md border px-2 py-1"
											>
												<AlertTriangle class="text-muted-foreground h-3 w-3" />
												<span class="text-muted-foreground text-xs font-medium">Disconnected</span>
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
