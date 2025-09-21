<script lang="ts">
	import type { ConnectionInfo } from '$lib/commands.svelte';
	import { Cable, Plus, Settings2, Unplug } from '@lucide/svelte';
	import { MenuItem, PredefinedMenuItem, Menu } from '@tauri-apps/api/menu';
	import type { SvelteSet } from 'svelte/reactivity';
	import IconCibPostgresql from '~icons/cib/postgresql';
	import IconSimpleIconsSqlite from '~icons/simple-icons/sqlite';
	import Button from './ui/button/button.svelte';

	interface Props {
		connections: ConnectionInfo[];
		selectedConnection: string | null;
		establishingConnections: SvelteSet<string>;
		onSelectConnection?: (connectionId: string) => void;
		onConnectToDatabase?: (connectionId: string) => void;
		onEditConnection?: (connection: ConnectionInfo) => void;
		onDeleteConnection?: (connectionId: string) => void;
		onDisconnectConnection?: (connectionId: string) => void;
		showConnectionForm: () => void;
	}

	const {
		connections,
		selectedConnection,
		establishingConnections,
		onSelectConnection,
		onConnectToDatabase,
		onEditConnection,
		onDisconnectConnection,
		onDeleteConnection,
		showConnectionForm
	}: Props = $props();

	let selectedConnectionInfo = $derived(connections.find((conn) => conn.id === selectedConnection));

	function selectConnection(connectionId: string) {
		onSelectConnection?.(connectionId);
	}

	function connectToDatabase(connectionId: string) {
		onConnectToDatabase?.(connectionId);
	}

	function editConnection(connection: ConnectionInfo) {
		onEditConnection?.(connection);
	}

	function deleteConnection(connectionId: string) {
		onDeleteConnection?.(connectionId);
	}

	function disconnectConnection(connectionId: string) {
		onDisconnectConnection?.(connectionId);
	}

	async function showContextMenu(event: MouseEvent, connection?: ConnectionInfo) {
		event.stopPropagation();
		event.preventDefault();

		try {
			const menu = await Menu.new();

			const separator = await PredefinedMenuItem.new({
				item: 'Separator'
			});

			const addItem = await MenuItem.new({
				text: 'Add Connection',
				action: showConnectionForm
			});

			await menu.append(addItem);
			await menu.append(separator);

			if (connection) {
				selectConnection(connection.id);

				const editItem = await MenuItem.new({
					text: 'Edit Connection',
					action: () => {
						editConnection(connection);
					}
				});

				let connectItem;
				if (connection.connected) {
					connectItem = await MenuItem.new({
						text: 'Disconnect',
						action: () => {
							disconnectConnection(connection.id);
						}
					});
				} else {
					connectItem = await MenuItem.new({
						text: 'Connect',
						action: () => {
							connectToDatabase(connection.id);
						}
					});
				}

				const deleteItem = await MenuItem.new({
					text: 'Delete Connection',
					action: () => {
						deleteConnection(connection.id);
					}
				});

				await menu.append(editItem);
				await menu.append(connectItem);
				await menu.append(separator);
				await menu.append(deleteItem);
			}

			await menu.popup();
		} catch (error) {
			console.error('Failed to show context menu:', error);
		}
	}
</script>

<div class="flex h-full flex-col space-y-2">
	<!-- Actions -->
	<div class="flex gap-1">
		<button
			title="Add"
			class="hover:bg-accent h-7 w-7 rounded-xs p-1"
			onclick={() => showConnectionForm()}
		>
			<Plus size="20" strokeWidth="1" />
		</button>
		<button
			disabled={!selectedConnectionInfo}
			title="Edit"
			class="enabled:hover:bg-accent h-7 w-7 rounded-xs p-1 disabled:opacity-40"
			onclick={() => {
				if (selectedConnectionInfo) {
					editConnection(selectedConnectionInfo);
				}
			}}
		>
			<Settings2 size="20" strokeWidth="1" />
		</button>
		<button
			disabled={!selectedConnectionInfo?.connected}
			title="Disconnect"
			class="enabled:hover:bg-error/20 enabled:text-error h-7 w-7 rounded-xs p-1 disabled:opacity-40"
			onclick={() => {
				if (selectedConnectionInfo) {
					disconnectConnection(selectedConnectionInfo.id);
				}
			}}
		>
			<Unplug size="20" strokeWidth="1" />
		</button>
	</div>

	<!-- List -->
	<div
		class="scrollable-container flex-1 space-y-1 overflow-y-auto"
		oncontextmenu={showContextMenu}
		role="menuitem"
		tabindex="-1"
	>
		{#if connections.length === 0}
			<div class="px-4 py-8 text-center">
				<div class="bg-muted/30 border-border/50 mb-3 inline-flex rounded-lg border p-3">
					<Cable class="text-muted-foreground/50 h-6 w-6" />
				</div>
				<p class="text-muted-foreground mb-1 text-xs font-medium">No connections yet</p>
				<p class="text-muted-foreground/70 text-xs">Add your first connection to get started</p>
			</div>
		{:else}
			{#each connections as connection (connection.id)}
				<Button
					variant="ghost"
					class="hover:bg-primary/20 w-full justify-start rounded-sm p-1 transition-all duration-200 {selectedConnection ===
					connection.id
						? 'bg-primary/20'
						: 'hover:bg-background'}"
					onclick={() => selectConnection(connection.id)}
					ondblclick={() => connectToDatabase(connection.id)}
					oncontextmenu={(event) => showContextMenu(event, connection)}
					data-context-menu="true"
				>
					<div class="flex w-full items-center gap-2.5">
						<div class="flex flex-shrink-0 items-center gap-2 pl-1">
							<!-- Connection status dot -->
							{#if connection.connected}
								<div class="h-1.5 w-1.5 rounded-full bg-green-500 shadow-sm"></div>
							{:else if establishingConnections.has(connection.id)}
								<div class="h-1.5 w-1.5 animate-pulse rounded-full bg-amber-500 shadow-sm"></div>
							{:else}
								<div class="h-1.5 w-1.5 rounded-full bg-gray-400"></div>
							{/if}

							{#if 'Postgres' in connection.database_type}
								<IconCibPostgresql class="h-4 w-4" />
							{:else if 'SQLite' in connection.database_type}
								<IconSimpleIconsSqlite class="h-4 w-4" />
							{/if}
						</div>
						<div class="text-foreground truncate text-sm font-medium">
							<div class="min-w-0 flex-1 text-left">
								{connection.name}
							</div>
							<div class="text-muted-foreground truncate font-mono text-xs">
								{#if 'Postgres' in connection.database_type}
									{connection.database_type.Postgres.connection_string
										.replace(/^postgresql?:\/\/[^@]*@/, '')
										.replace(/\/[^?]*/, '')}
								{:else if 'SQLite' in connection.database_type}
									{connection.database_type.SQLite.db_path.split('/').pop() ||
										connection.database_type.SQLite.db_path}
								{/if}
							</div>
						</div>
					</div>
				</Button>
			{/each}
		{/if}
	</div>
</div>
