<script lang="ts">
	import {
		Cable,
		Plus,
		ChevronLeft,
		ChevronRight,
		FileJson,
		TableProperties,
		History
	} from '@lucide/svelte';
	import IconCibPostgresql from '~icons/cib/postgresql';
	import IconSimpleIconsSqlite from '~icons/simple-icons/sqlite';
	import { Button } from '$lib/components/ui/button';
	import { Accordion, AccordionItem, AccordionContent } from '$lib/components/ui/accordion';
	import { Menu, MenuItem, PredefinedMenuItem } from '@tauri-apps/api/menu';
	import DatabaseSchemaItems from './DatabaseSchemaItems.svelte';
	import Logo from './Logo.svelte';
	import type {
		ConnectionInfo,
		Script,
		DatabaseSchema,
		QueryHistoryEntry
	} from '$lib/commands.svelte';
	import { SvelteSet } from 'svelte/reactivity';

	interface Props {
		connections: ConnectionInfo[];
		selectedConnection: string | null;
		establishingConnections: SvelteSet<string>;
		scripts: Script[];
		activeScriptId: number | null;
		unsavedChanges: SvelteSet<number>;
		databaseSchema: DatabaseSchema | null;
		loadingSchema: boolean;
		queryHistory: QueryHistoryEntry[];

		//Bound props
		isSidebarCollapsed?: boolean;
		isConnectionsAccordionOpen?: boolean;
		isScriptsAccordionOpen?: boolean;
		isHistoryAccordionOpen?: boolean;
		isItemsAccordionOpen?: boolean;

		onToggleSidebar?: () => void;
		onSelectConnection?: (connectionId: string) => void;
		onConnectToDatabase?: (connectionId: string) => void;
		onShowConnectionForm?: () => void;
		onEditConnection?: (connection: ConnectionInfo) => void;
		onDeleteConnection?: (connectionId: string) => void;
		onDisconnectConnection?: (connectionId: string) => void;
		onSelectScript?: (script: Script) => void;
		onCreateNewScript?: () => void;
		onDeleteScript?: (script: Script) => void;
		onTableClick?: (tableName: string, schema: string) => void;
		onLoadFromHistory?: (historyQuery: string) => void;
	}

	let {
		connections,
		selectedConnection,
		establishingConnections,
		scripts,
		activeScriptId,
		unsavedChanges,
		databaseSchema,
		loadingSchema,
		queryHistory,

		isSidebarCollapsed = $bindable(false),
		isConnectionsAccordionOpen = $bindable(true),
		isScriptsAccordionOpen = $bindable(false),
		isHistoryAccordionOpen = $bindable(false),
		isItemsAccordionOpen = $bindable(false),

		onToggleSidebar,
		onSelectConnection,
		onConnectToDatabase,
		onShowConnectionForm,
		onEditConnection,
		onDeleteConnection,
		onDisconnectConnection,
		onSelectScript,
		onCreateNewScript,
		onDeleteScript,
		onTableClick,
		onLoadFromHistory
	}: Props = $props();

	function toggleSidebar() {
		isSidebarCollapsed = !isSidebarCollapsed;
		onToggleSidebar?.();
	}

	function selectConnection(connectionId: string) {
		onSelectConnection?.(connectionId);
	}

	function connectToDatabase(connectionId: string) {
		onConnectToDatabase?.(connectionId);
	}

	function showConnectionForm() {
		onShowConnectionForm?.();
	}

	function selectScript(script: Script) {
		onSelectScript?.(script);
	}

	function createNewScript() {
		onCreateNewScript?.();
	}

	function deleteScript(script: Script) {
		onDeleteScript?.(script);
	}

	function handleTableClick(tableName: string, schema: string) {
		onTableClick?.(tableName, schema);
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

	async function showContextMenu(event: MouseEvent, connection: ConnectionInfo) {
		event.preventDefault();

		try {
			const menu = await Menu.new();

			const editItem = await MenuItem.new({
				text: 'Edit Connection',
				action: () => {
					console.log('Edit clicked');
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

			const separator = await PredefinedMenuItem.new({
				item: 'Separator'
			});

			await menu.append(editItem);
			await menu.append(connectItem);
			await menu.append(separator);
			await menu.append(deleteItem);

			await menu.popup();
		} catch (error) {
			console.error('Failed to show context menu:', error);
			console.error('Error details:', error);
		}
	}
</script>

<div
	class="bg-sidebar/80 glass-subtle border-sidebar-border relative flex h-full flex-col border-r"
>
	{#if isSidebarCollapsed}
		<div class="border-sidebar-border/50 border-b p-3">
			<div class="flex flex-col items-center gap-3">
				<button
					class="hover:bg-sidebar-accent/80 rounded-lg p-2 transition-all duration-200 hover:shadow-md"
					onclick={toggleSidebar}
					title="Expand sidebar"
				>
					<ChevronRight class="text-sidebar-foreground/70 h-4 w-4" />
				</button>
				<Button
					size="icon-sm"
					variant="outline"
					class="shadow-md hover:shadow-lg"
					onclick={showConnectionForm}
					title="Add Connection"
				>
					<Plus class="h-4 w-4" />
				</Button>
			</div>
		</div>

		<div class="flex flex-1 flex-col items-center justify-start space-y-4 p-4">
			<!-- Connections icon -->
			<button
				class="group flex h-12 w-12 items-center justify-center rounded-lg transition-all duration-200 ease-out hover:bg-white/3 dark:hover:bg-white/5"
				onclick={() => {
					isSidebarCollapsed = false;
					isConnectionsAccordionOpen = true;
					isScriptsAccordionOpen = false;
					isHistoryAccordionOpen = false;
					isItemsAccordionOpen = false;
				}}
				title="Connections"
			>
				<Cable
					class="text-sidebar-foreground/70 group-hover:text-primary/90 h-5 w-5 transition-colors duration-200"
				/>
			</button>

			<!-- Scripts icon -->
			<button
				class="group flex h-12 w-12 items-center justify-center rounded-lg transition-all duration-200 ease-out hover:bg-white/3 dark:hover:bg-white/5"
				onclick={() => {
					isSidebarCollapsed = false;
					isConnectionsAccordionOpen = false;
					isScriptsAccordionOpen = true;
					isHistoryAccordionOpen = false;
					isItemsAccordionOpen = false;
				}}
				title="Scripts"
			>
				<FileJson
					class="text-sidebar-foreground/70 group-hover:text-primary/90 h-5 w-5 transition-colors duration-200"
				/>
			</button>

			<button
				class="group flex h-12 w-12 items-center justify-center rounded-lg transition-all duration-200 ease-out hover:bg-white/3 dark:hover:bg-white/5"
				onclick={() => {
					isSidebarCollapsed = false;
					isConnectionsAccordionOpen = false;
					isScriptsAccordionOpen = false;
					isHistoryAccordionOpen = true;
					isItemsAccordionOpen = false;
				}}
				title="Query History"
			>
				<History
					class="text-sidebar-foreground/70 group-hover:text-primary/90 h-5 w-5 transition-colors duration-200"
				/>
			</button>

			<!-- items icon -->
			<button
				class="group flex h-12 w-12 items-center justify-center rounded-lg transition-all duration-200 ease-out hover:bg-white/3 dark:hover:bg-white/5"
				onclick={() => {
					isSidebarCollapsed = false;
					isConnectionsAccordionOpen = false;
					isScriptsAccordionOpen = false;
					isHistoryAccordionOpen = false;
					isItemsAccordionOpen = true;
				}}
				title="Database Items"
			>
				<TableProperties
					class="text-sidebar-foreground/70 group-hover:text-primary/90 h-5 w-5 transition-colors duration-200"
				/>
			</button>
		</div>
	{:else}
		<!-- expanded sidebar content -->
		<button
			class="hover:bg-sidebar-accent/80 absolute top-6 right-4 z-10 rounded-lg p-2 transition-all duration-200 hover:shadow-md"
			onclick={toggleSidebar}
			title="Collapse sidebar"
		>
			<ChevronLeft class="text-sidebar-foreground/70 h-4 w-4" />
		</button>

		<div class="border-sidebar-border/50 border-b p-6">
			<div class="flex items-center gap-4">
				<Logo
					class="text-primary/90 hover:text-primary drop-shadow-lg transition-all duration-300 hover:scale-105"
					size="md"
				/>
				<h1 class="text-sidebar-foreground text-xl font-bold">PgPad</h1>
			</div>
		</div>

		<div class="scrollable-container scrollbar-none flex-1 overflow-y-auto p-2">
			<Accordion>
				<!-- Connections accordion -->
				<AccordionItem title="Connections" icon={Cable} bind:open={isConnectionsAccordionOpen}>
					<AccordionContent>
						<div class="space-y-3">
							<Button
								variant="ghost"
								class="mb-4 w-full gap-2 rounded-md border border-dashed border-gray-300/60 bg-transparent p-2.5 text-sm transition-all duration-200 hover:border-gray-400/80 hover:bg-black/3 dark:border-gray-600/60 dark:hover:border-gray-500/80 dark:hover:bg-white/3"
								onclick={showConnectionForm}
							>
								<Plus class="h-4 w-4" />
								Add Connection
							</Button>

							<div class="space-y-1">
								{#if connections.length === 0}
									<div class="px-4 py-8 text-center">
										<div
											class="bg-muted/30 border-border/50 mb-3 inline-flex rounded-lg border p-3"
										>
											<Cable class="text-muted-foreground/50 h-6 w-6" />
										</div>
										<p class="text-muted-foreground mb-1 text-xs font-medium">No connections yet</p>
										<p class="text-muted-foreground/70 text-xs">
											Add your first connection to get started
										</p>
									</div>
								{:else}
									{#each connections as connection (connection.id)}
										<Button
											variant="ghost"
											class="h-auto w-full justify-start rounded-none p-3 transition-all duration-200 hover:bg-black/3 dark:hover:bg-white/3 {selectedConnection ===
											connection.id
												? 'dark:bg-primary/20 border-l-2 border-l-blue-500 bg-blue-50/50 dark:border-l-blue-400'
												: 'border-l-2 border-l-transparent'}"
											onclick={() => selectConnection(connection.id)}
											ondblclick={() => connectToDatabase(connection.id)}
											oncontextmenu={(event) => showContextMenu(event, connection)}
											data-context-menu="true"
										>
											<div class="flex w-full items-center gap-2.5">
												<div class="flex flex-shrink-0 items-center gap-2">
													<!-- Connection status dot -->
													{#if connection.connected}
														<div class="h-2 w-2 rounded-full bg-green-500 shadow-sm"></div>
													{:else if establishingConnections.has(connection.id)}
														<div
															class="h-2 w-2 animate-pulse rounded-full bg-amber-500 shadow-sm"
														></div>
													{:else}
														<div class="h-2 w-2 rounded-full bg-gray-400"></div>
													{/if}

													{#if 'Postgres' in connection.database_type}
														<IconCibPostgresql class="h-4 w-4" />
													{:else if 'SQLite' in connection.database_type}
														<IconSimpleIconsSqlite class="h-4 w-4" />
													{/if}
												</div>

												<div class="min-w-0 flex-1 text-left">
													<div class="text-foreground mb-0.5 truncate text-sm font-medium">
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
					</AccordionContent>
				</AccordionItem>

				<!-- Scripts accordion -->
				<AccordionItem title="Scripts" icon={FileJson} bind:open={isScriptsAccordionOpen}>
					<AccordionContent>
						<div class="space-y-3">
							<Button
								class="w-full justify-start gap-2 rounded-md border border-dashed border-gray-300/60 bg-transparent p-2.5 text-sm transition-all duration-200 hover:border-gray-400/80 hover:bg-black/3 dark:border-gray-600/60 dark:hover:border-gray-500/80 dark:hover:bg-white/3"
								variant="ghost"
								onclick={createNewScript}
							>
								<Plus class="h-4 w-4" />
								New Script
							</Button>

							<div class="space-y-1">
								{#if scripts.length === 0}
									<div class="px-4 py-8 text-center">
										<div
											class="bg-muted/30 border-border/50 mb-3 inline-flex rounded-lg border p-3"
										>
											<FileJson class="text-muted-foreground/50 h-6 w-6" />
										</div>
										<p class="text-muted-foreground mb-1 text-xs font-medium">
											No saved scripts yet
										</p>
										<p class="text-muted-foreground/70 text-xs">
											Save your SQL queries to access them later
										</p>
									</div>
								{:else}
									{#each scripts as script (script.id)}
										<div class="group relative">
											<Button
												variant="ghost"
												class="h-auto w-full justify-start rounded-none p-3 transition-all duration-200 hover:bg-black/3 dark:hover:bg-white/3 {activeScriptId ===
												script.id
													? 'dark:bg-primary/20 border-l-2 border-l-blue-500 bg-blue-50/50 dark:border-l-blue-400'
													: 'border-l-2 border-l-transparent'}"
												onclick={() => selectScript(script)}
											>
												<div class="flex w-full items-start gap-3">
													<div class="mt-1 flex-shrink-0">
														<FileJson class="text-muted-foreground h-3 w-3" />
													</div>

													<div class="min-w-0 flex-1 text-left">
														<div class="text-foreground mb-1 truncate text-xs font-medium">
															{script.name}
															{#if activeScriptId === script.id && unsavedChanges.has(script.id)}
																<span class="text-orange-500">*</span>
															{/if}
														</div>
														<div class="text-muted-foreground/80 truncate text-xs">
															Modified {new Date(script.updated_at * 1000).toLocaleDateString()}
														</div>
													</div>
												</div>
											</Button>

											<!-- Delete button -->
											<button
												class="absolute top-2 right-2 rounded p-1 opacity-0 transition-all group-hover:opacity-100 hover:bg-red-100 hover:text-red-600"
												onclick={(e) => {
													e.stopPropagation();
													deleteScript(script);
												}}
												title="Delete script"
												aria-label="Delete script"
											>
												<svg class="h-3 w-3" fill="currentColor" viewBox="0 0 20 20">
													<path
														fill-rule="evenodd"
														d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
														clip-rule="evenodd"
													></path>
												</svg>
											</button>
										</div>
									{/each}
								{/if}
							</div>
						</div>
					</AccordionContent>
				</AccordionItem>

				<AccordionItem title="History" icon={History} bind:open={isHistoryAccordionOpen}>
					<AccordionContent>
						<div class="space-y-3">
							{#if queryHistory.length > 0}
								<div class="space-y-2">
									{#each queryHistory as historyItem (historyItem.id)}
										<button
											type="button"
											class="group hover:bg-muted/30 w-full cursor-pointer rounded-lg border p-3 text-left transition-colors"
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
												<span
													class="text-primary text-xs font-medium opacity-0 group-hover:opacity-100"
												>
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
										<div
											class="bg-muted/20 mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-full"
										>
											<History class="text-muted-foreground/50 h-6 w-6" />
										</div>
										<p class="text-sm font-medium">No query history</p>
										<p class="text-muted-foreground/70 mt-1 text-xs">
											Execute queries to see history here
										</p>
									</div>
								</div>
							{/if}
						</div>
					</AccordionContent>
				</AccordionItem>

				<!-- database items accordion -->
				<AccordionItem title="Items" icon={TableProperties} bind:open={isItemsAccordionOpen}>
					<AccordionContent>
						<DatabaseSchemaItems
							{databaseSchema}
							{loadingSchema}
							{selectedConnection}
							onTableClick={handleTableClick}
						/>
					</AccordionContent>
				</AccordionItem>
			</Accordion>
		</div>
	{/if}
</div>
