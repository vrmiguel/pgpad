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

<div class="flex-1 overflow-y-auto">
	<div class="p-2">
		<h2 class="text-sm font-medium text-gray-500 mb-3 px-2">CONNECTIONS</h2>
		
		{#if connections.length === 0}
			<div class="text-center py-8 text-gray-500">
				<Database class="w-8 h-8 mx-auto mb-2 opacity-50" />
				<p class="text-sm">No connections yet</p>
				<p class="text-xs text-gray-400 mt-1">Add your first connection to get started</p>
			</div>
		{:else}
			<div class="space-y-1">
				{#each connections as connection (connection.id)}
					<Button
						variant={selectedConnection === connection.id ? "secondary" : "ghost"}
						class="w-full justify-start p-3 h-auto"
						onclick={() => onSelect(connection.id)}
						ondblclick={() => onConnect?.(connection.id)}
					>
						<div class="flex items-start gap-3 w-full">
							<div class="flex-shrink-0 mt-0.5">
								{#if connection.connected}
									<Circle class="w-3 h-3 fill-green-500 text-green-500" />
								{:else if establishingConnections.has(connection.id)}
									<Loader2 class="w-3 h-3 text-blue-500 animate-spin" />
								{:else}
									<Circle class="w-3 h-3 fill-gray-300 text-gray-300" />
								{/if}
							</div>
							
							<div class="flex-1 text-left min-w-0">
								<div class="font-medium text-sm text-gray-900 truncate">
									{connection.name}
								</div>
								<div class="text-xs text-gray-500 mt-0.5 truncate">
									{connection.connection_string.replace(/^postgresql?:\/\/[^@]*@/, '').replace(/\/[^?]*/, '')}
								</div>
								<div class="text-xs text-gray-400 mt-0.5 truncate">
									{connection.connection_string.split('/').pop()?.split('?')[0] || 'database'}
								</div>
								{#if !connection.connected}
									<div class="flex items-center gap-1 mt-1">
										{#if establishingConnections.has(connection.id)}
											<span class="text-xs text-blue-600">Connecting...</span>
										{:else}
											<AlertTriangle class="w-3 h-3 text-amber-500" />
											<span class="text-xs text-amber-600">Disconnected</span>
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