<script lang="ts">
	import { Database, Plus, ChevronLeft, ChevronRight, FileText, Table } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import { Accordion, AccordionItem, AccordionContent } from '$lib/components/ui/accordion';
	import DatabaseSchemaItems from './DatabaseSchemaItems.svelte';
	import type { ConnectionInfo, Script, DatabaseSchema } from '$lib/commands.svelte';
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

		//Bound props
		isSidebarCollapsed?: boolean;
		isConnectionsAccordionOpen?: boolean;
		isScriptsAccordionOpen?: boolean;
		isItemsAccordionOpen?: boolean;

		onToggleSidebar?: () => void;
		onSelectConnection?: (connectionId: string) => void;
		onConnectToDatabase?: (connectionId: string) => void;
		onShowConnectionForm?: () => void;
		onSelectScript?: (script: Script) => void;
		onCreateNewScript?: () => void;
		onDeleteScript?: (script: Script) => void;
		onTableClick?: (tableName: string, schema: string) => void;
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

		isSidebarCollapsed = $bindable(false),
		isConnectionsAccordionOpen = $bindable(true),
		isScriptsAccordionOpen = $bindable(false),
		isItemsAccordionOpen = $bindable(false),

		onToggleSidebar,
		onSelectConnection,
		onConnectToDatabase,
		onShowConnectionForm,
		onSelectScript,
		onCreateNewScript,
		onDeleteScript,
		onTableClick
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
</script>

<div
	class="bg-sidebar/80 glass-subtle border-sidebar-border relative flex h-full flex-col border-r"
>
	{#if isSidebarCollapsed}
		<button
			class="hover:bg-sidebar-accent/80 absolute top-4 right-2 z-10 rounded-lg p-2 transition-all duration-200 hover:shadow-md"
			onclick={toggleSidebar}
			title="Expand sidebar"
		>
			<ChevronRight class="text-sidebar-foreground/70 h-4 w-4" />
		</button>

		<div class="border-sidebar-border/50 border-b p-3">
			<div class="flex flex-col items-center gap-3">
				<div class="bg-primary/10 border-primary/20 rounded-lg border p-2">
					<Database class="text-primary h-6 w-6" />
				</div>
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
				class="hover:bg-sidebar-accent/80 flex h-12 w-12 items-center justify-center rounded-lg transition-all duration-200 hover:shadow-md"
				onclick={() => {
					isSidebarCollapsed = false;
					isConnectionsAccordionOpen = true;
					isScriptsAccordionOpen = false;
					isItemsAccordionOpen = false;
				}}
				title="Connections"
			>
				<Database class="text-sidebar-foreground/70 h-5 w-5" />
			</button>

			<!-- Scripts icon -->
			<button
				class="hover:bg-sidebar-accent/80 flex h-12 w-12 items-center justify-center rounded-lg transition-all duration-200 hover:shadow-md"
				onclick={() => {
					isSidebarCollapsed = false;
					isConnectionsAccordionOpen = false;
					isScriptsAccordionOpen = true;
					isItemsAccordionOpen = false;
				}}
				title="Scripts"
			>
				<FileText class="text-sidebar-foreground/70 h-5 w-5" />
			</button>

			<!-- items icon -->
			<button
				class="hover:bg-sidebar-accent/80 flex h-12 w-12 items-center justify-center rounded-lg transition-all duration-200 hover:shadow-md"
				onclick={() => {
					isSidebarCollapsed = false;
					isConnectionsAccordionOpen = false;
					isScriptsAccordionOpen = false;
					isItemsAccordionOpen = true;
				}}
				title="Database Items"
			>
				<Table class="text-sidebar-foreground/70 h-5 w-5" />
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
			<div class="flex items-center gap-3">
				<div class="bg-primary/10 border-primary/20 rounded-lg border p-2">
					<Database class="text-primary h-6 w-6" />
				</div>
				<h1 class="text-sidebar-foreground text-xl font-bold">PgPad</h1>
			</div>
		</div>

		<div class="flex-1 overflow-y-auto p-4">
			<Accordion>
				<!-- Connections accordion -->
				<AccordionItem title="Connections" icon={Database} bind:open={isConnectionsAccordionOpen}>
					<AccordionContent>
						<div class="space-y-3">
							<Button
								variant="ghost"
								class="mb-3 w-full gap-2 rounded-lg border border-dashed border-gray-300 bg-black/5 transition-all duration-200 hover:border-gray-400 hover:bg-black/10 dark:border-gray-600 dark:bg-white/5 dark:hover:border-gray-500 dark:hover:bg-white/10"
								onclick={showConnectionForm}
							>
								<Plus class="h-4 w-4" />
								Add Connection
							</Button>

							<div class="space-y-2">
								{#if connections.length === 0}
									<div class="px-4 py-8 text-center">
										<div
											class="bg-muted/30 border-border/50 mb-3 inline-flex rounded-lg border p-3"
										>
											<Database class="text-muted-foreground/50 h-6 w-6" />
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
											class="h-auto w-full justify-start rounded-lg p-2.5 transition-all duration-200 hover:bg-black/5 dark:hover:bg-white/5 {selectedConnection ===
											connection.id
												? 'border border-blue-200/50 bg-blue-50 dark:border-blue-800/50 dark:bg-blue-950/30'
												: 'border border-transparent'}"
											onclick={() => selectConnection(connection.id)}
											ondblclick={() => connectToDatabase(connection.id)}
										>
											<div class="flex w-full items-center gap-2.5">
												<div class="flex-shrink-0">
													{#if connection.connected}
														<div class="h-2 w-2 rounded-full bg-green-500 shadow-sm"></div>
													{:else if establishingConnections.has(connection.id)}
														<div
															class="h-2 w-2 animate-pulse rounded-full bg-amber-500 shadow-sm"
														></div>
													{:else}
														<div class="h-2 w-2 rounded-full bg-gray-400"></div>
													{/if}
												</div>

												<div class="min-w-0 flex-1 text-left">
													<div class="text-foreground mb-0.5 truncate text-sm font-medium">
														{connection.name}
													</div>
													<div class="text-muted-foreground truncate font-mono text-xs">
														{connection.connection_string
															.replace(/^postgresql?:\/\/[^@]*@/, '')
															.replace(/\/[^?]*/, '')}
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
				<AccordionItem title="Scripts" icon={FileText} bind:open={isScriptsAccordionOpen}>
					<AccordionContent>
						<div class="space-y-3">
							<Button
								class="w-full justify-start gap-2 shadow-sm hover:shadow-md"
								variant="outline"
								onclick={createNewScript}
							>
								<Plus class="h-4 w-4" />
								New Script
							</Button>

							<div class="space-y-2">
								{#if scripts.length === 0}
									<div class="px-4 py-8 text-center">
										<div
											class="bg-muted/30 border-border/50 mb-3 inline-flex rounded-lg border p-3"
										>
											<FileText class="text-muted-foreground/50 h-6 w-6" />
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
												variant={activeScriptId === script.id ? 'secondary' : 'ghost'}
												class="h-auto w-full justify-start p-3 shadow-sm transition-all duration-200 hover:shadow-md {activeScriptId ===
												script.id
													? 'bg-primary/10 border-primary/20 border shadow-md'
													: ''}"
												onclick={() => selectScript(script)}
											>
												<div class="flex w-full items-start gap-3">
													<div class="mt-1 flex-shrink-0">
														<FileText class="text-muted-foreground h-3 w-3" />
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

				<!-- database items accordion -->
				<AccordionItem title="Items" icon={Table} bind:open={isItemsAccordionOpen}>
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
