<script lang="ts">
	import {
		Database,
		Plus,
		Play,
		Save,
		ChevronLeft,
		ChevronRight,
		FileText,
		Table
	} from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import { ResizablePaneGroup, ResizablePane, ResizableHandle } from '$lib/components/ui/resizable';
	import { Accordion, AccordionItem, AccordionContent } from '$lib/components/ui/accordion';
	import ConnectionSidebar from './ConnectionSidebar.svelte';
	import SqlEditor from './SqlEditor.svelte';
	import ConnectionForm from './ConnectionForm.svelte';
	import ScriptTabs from './ScriptTabs.svelte';
	import DatabaseSchemaItems from './DatabaseSchemaItems.svelte';
	import TableBrowseModal from './TableBrowseModal.svelte';
	import {
		DatabaseCommands,
		type ConnectionInfo,
		type ConnectionConfig,
		type Script,
		type DatabaseSchema
	} from '$lib/commands.svelte';
	import { onMount } from 'svelte';

	interface Props {
		currentConnection?: {
			name: string;
			connected: boolean;
		} | null;
		isConnecting?: boolean;
	}

	let { currentConnection = $bindable(), isConnecting = $bindable() }: Props = $props();

	let showConnectionForm = $state(false);
	let selectedConnection = $state<string | null>(null);
	let connections = $state<ConnectionInfo[]>([]);
	let isRunningQuery = $state(false);
	let sqlEditorRef = $state<any>();
	let establishingConnections = $state<Set<string>>(new Set());

	let scripts = $state<Script[]>([]);
	let openScripts = $state<Script[]>([]);
	let activeScriptId = $state<number | null>(null);
	let unsavedChanges = $state<Set<number>>(new Set());
	let scriptContents = $state<Map<number, string>>(new Map());
	let currentEditorContent = $state<string>('');
	// Scripts not yet persisted in SQLite
	let newScripts = $state<Set<number>>(new Set());
	// Let's use negative IDs for unpersisted scripts
	let nextTempId = $state(-1);

	let isSidebarCollapsed = $state(false);
	let lastResizeTime = $state(0);

	let databaseSchema = $state<DatabaseSchema | null>(null);
	let loadingSchema = $state(false);
	let lastLoadedSchemaConnectionId = $state<string | null>(null);
	let isItemsAccordionOpen = $state(false);

	let tableBrowseModalOpen = $state(false);
	let selectedTableName = $state('');
	let selectedTableSchema = $state('');

	// Auto-collapse if resized below 8%
	const COLLAPSE_THRESHOLD = 12;
	// Auto-expand if resized above 10%
	const EXPAND_THRESHOLD = 10;

	$effect(() => {
		const connId = selectedConnection;
		const allConnections = connections;
		const connecting = establishingConnections;

		if (connId) {
			const connection = allConnections.find((c) => c.id === connId);
			if (connection) {
				currentConnection = {
					name: connection.name,
					connected: connection.connected
				};
				isConnecting = connecting.has(connection.id);
			}
		} else {
			currentConnection = null;
			isConnecting = false;
		}
	});

	$effect(() => {
		const connId = selectedConnection;
		const allConnections = connections;

		if (connId) {
			const connection = allConnections.find((c) => c.id === connId);
			if (connection?.connected) {
				loadDatabaseSchemaIfNeeded(connId);
			} else {
				databaseSchema = null;
				lastLoadedSchemaConnectionId = null;
			}
		} else {
			databaseSchema = null;
			lastLoadedSchemaConnectionId = null;
		}
	});

	// Track unsaved changes for active script
	$effect(() => {
		if (activeScriptId !== null && currentEditorContent !== undefined) {
			const savedContent = scripts.find((s) => s.id === activeScriptId)?.query_text || '';
			const isNewScript = newScripts.has(activeScriptId);

			// For new scripts, only show unsaved changes if there's actual content
			// For existing scripts, show unsaved changes if content differs from saved version
			const shouldShowUnsaved = isNewScript
				? currentEditorContent.length > 0
				: currentEditorContent !== savedContent;

			if (shouldShowUnsaved) {
				if (!unsavedChanges.has(activeScriptId)) {
					unsavedChanges = new Set([...unsavedChanges, activeScriptId]);
				}
			} else {
				if (unsavedChanges.has(activeScriptId)) {
					unsavedChanges = new Set([...unsavedChanges].filter((id) => id !== activeScriptId));
				}
			}
		}
	});

	// Update currentEditorContent when switching scripts
	$effect(() => {
		if (activeScriptId !== null) {
			const content =
				scriptContents.get(activeScriptId) ||
				scripts.find((s) => s.id === activeScriptId)?.query_text ||
				'';
			currentEditorContent = content;
		} else {
			currentEditorContent = '';
		}
	});

	function handleEditorContentChange(newContent: string) {
		currentEditorContent = newContent;
		if (activeScriptId !== null) {
			scriptContents.set(activeScriptId, newContent);
		}
	}

	function handlePaneResize(sizes: number[]) {
		const now = Date.now();
		lastResizeTime = now;

		// Some 'debounce' logic because the cursor was getting stuck for some reason
		setTimeout(() => {
			if (now === lastResizeTime && sizes.length >= 2) {
				const sidebarSize = sizes[0];

				if (!isSidebarCollapsed && sidebarSize < COLLAPSE_THRESHOLD) {
					isSidebarCollapsed = true;
				} else if (isSidebarCollapsed && sidebarSize > EXPAND_THRESHOLD) {
					isSidebarCollapsed = false;
				}
			}
		}, 150);
	}

	function openScript(script: Script) {
		// Add to open scripts if not already open
		if (!openScripts.find((s) => s.id === script.id)) {
			openScripts.push(script);
		}

		// Set as active and initialize content
		activeScriptId = script.id;
		scriptContents.set(script.id, script.query_text);

		// Update the editor content
		if (sqlEditorRef) {
			sqlEditorRef.setContent(script.query_text);
		}
	}

	function switchToTab(scriptId: number) {
		// Save current script content before switching
		if (activeScriptId !== null) {
			scriptContents.set(activeScriptId, currentEditorContent);
		}

		// Switch to new script
		activeScriptId = scriptId;
		const script = scripts.find((s) => s.id === scriptId);
		if (script) {
			const content = scriptContents.get(scriptId) || script.query_text;
			if (sqlEditorRef) {
				sqlEditorRef.setContent(content);
			}
		}
	}

	function closeTab(scriptId: number) {
		// Remove from open scripts
		openScripts = openScripts.filter((s) => s.id !== scriptId);

		// Clean up state
		scriptContents.delete(scriptId);
		unsavedChanges = new Set([...unsavedChanges].filter((id) => id !== scriptId));
		newScripts.delete(scriptId);

		// If closing active tab, switch to another or clear active
		if (activeScriptId === scriptId) {
			if (openScripts.length > 0) {
				// Switch to the last remaining tab
				const lastScript = openScripts[openScripts.length - 1];
				switchToTab(lastScript.id);
			} else {
				// No tabs left
				activeScriptId = null;
				if (sqlEditorRef) {
					sqlEditorRef.setContent('');
				}
			}
		}
	}

	function selectScript(script: Script) {
		openScript(script);
	}

	async function loadQueryFromHistory(historyQuery: string) {
		const name = generateScriptName();
		const tempId = nextTempId--;

		const newScript: Script = {
			id: tempId,
			name,
			description: null,
			query_text: historyQuery,
			connection_id: null,
			tags: null,
			created_at: Date.now() / 1000,
			updated_at: Date.now() / 1000,
			favorite: false
		};

		scripts.push(newScript);
		// Mark as new/unsaved
		newScripts.add(tempId);
		openScript(newScript);
	}

	onMount(async () => {
		try {
			await DatabaseCommands.initializeConnections();
			await loadConnections();
			await loadScripts();

			// Create a new script on startup if no scripts are open
			if (openScripts.length === 0) {
				// Reuse "Untitled Script" on startup
				const existingUntitledScript = scripts.find((s) => s.name === 'Untitled Script');

				if (existingUntitledScript) {
					openScript(existingUntitledScript);
				} else {
					await createNewScript();
				}
			}
		} catch (error) {
			console.error('Failed to initialize connections:', error);
		}
	});

	// Handle saving with Ctrl+S
	function handleKeydown(event: KeyboardEvent) {
		if ((event.ctrlKey || event.metaKey) && event.key === 's') {
			event.preventDefault();
			saveCurrentScript();
		}
	}

	async function loadConnections() {
		try {
			connections = await DatabaseCommands.getConnections();
		} catch (error) {
			console.error('Failed to load connections:', error);
		}
	}

	async function addConnection(config: ConnectionConfig) {
		try {
			const newConnection = await DatabaseCommands.addConnection(config);
			connections.push(newConnection);
			showConnectionForm = false;
		} catch (error) {
			console.error('Failed to add connection:', error);
		}
	}

	function selectConnection(connectionId: string) {
		selectedConnection = connectionId;
	}

	async function connectToDatabase(connectionId: string) {
		establishingConnections = new Set([...establishingConnections, connectionId]);

		try {
			const success = await DatabaseCommands.connectToDatabase(connectionId);
			if (success) {
				// Update the connection status
				await loadConnections();
				// Load schema after successful connection
				if (selectedConnection === connectionId) {
					await loadDatabaseSchemaForced();
				}
			}
		} catch (error) {
			console.error('Failed to connect:', error);
		} finally {
			establishingConnections = new Set(
				[...establishingConnections].filter((id) => id !== connectionId)
			);
		}
	}

	async function loadDatabaseSchemaIfNeeded(connectionId: string) {
		// Don't reload if already loaded for this connection or currently loading
		if (lastLoadedSchemaConnectionId === connectionId || loadingSchema) {
			return;
		}

		try {
			loadingSchema = true;
			databaseSchema = await DatabaseCommands.getDatabaseSchema(connectionId);
			lastLoadedSchemaConnectionId = connectionId;
		} catch (error) {
			console.error('Failed to load database schema:', error);
			databaseSchema = null;
			lastLoadedSchemaConnectionId = null;
		} finally {
			loadingSchema = false;
		}
	}

	// Force load schema regardless of connection state (used after successful connection)
	async function loadDatabaseSchemaForced() {
		if (!selectedConnection || loadingSchema) return;

		try {
			loadingSchema = true;
			// Reset the last loaded ID to force reload
			lastLoadedSchemaConnectionId = null;
			databaseSchema = await DatabaseCommands.getDatabaseSchema(selectedConnection);
			lastLoadedSchemaConnectionId = selectedConnection;
		} catch (error) {
			console.error('Failed to load database schema:', error);
			databaseSchema = null;
			lastLoadedSchemaConnectionId = null;
		} finally {
			loadingSchema = false;
		}
	}

	function handleTableClick(tableName: string, schema: string) {
		selectedTableName = tableName;
		selectedTableSchema = schema;
		tableBrowseModalOpen = true;
	}

	function closeTableBrowseModal() {
		tableBrowseModalOpen = false;
		selectedTableName = '';
		selectedTableSchema = '';
	}

	// Sidebar toggle functionality
	function toggleSidebar() {
		isSidebarCollapsed = !isSidebarCollapsed;
	}

	async function loadScripts() {
		try {
			scripts = await DatabaseCommands.getScripts();
		} catch (error) {
			console.error('Failed to load scripts:', error);
		}
	}

	function generateScriptName(): string {
		const now = new Date();
		const timestamp = now.toISOString().split('T')[0]; // YYYY-MM-DD
		const existingUntitled = scripts.filter((s) => s.name.startsWith('Untitled Script')).length;
		return existingUntitled === 0 ? 'Untitled Script' : `Untitled Script ${existingUntitled + 1}`;
	}

	async function createNewScript() {
		try {
			const name = generateScriptName();
			const content = '';
			const tempId = nextTempId--; // Use negative ID for new scripts

			const newScript: Script = {
				id: tempId,
				name,
				description: null,
				query_text: content,
				connection_id: null,
				tags: null,
				created_at: Date.now() / 1000,
				updated_at: Date.now() / 1000,
				favorite: false
			};

			scripts.push(newScript);
			// Mark as new/unsaved
			newScripts.add(tempId);
			openScript(newScript);
		} catch (error) {
			console.error('Failed to create new script:', error);
		}
	}

	async function saveCurrentScript() {
		if (activeScriptId === null) {
			// Create a new script if none is selected
			await createNewScript();
			return;
		}

		try {
			const content = currentEditorContent;
			const currentScript = scripts.find((s) => s.id === activeScriptId);
			if (!currentScript) return;

			const isNewScript = newScripts.has(activeScriptId);

			if (isNewScript) {
				const scriptId = await DatabaseCommands.saveScript(
					currentScript.name,
					content,
					currentScript.connection_id || undefined,
					currentScript.description || undefined
				);

				// Update the script with the real database ID
				const updatedScript = {
					...currentScript,
					id: scriptId,
					query_text: content,
					updated_at: Date.now() / 1000
				};

				// Update all references to use the new ID
				const scriptIndex = scripts.findIndex((s) => s.id === activeScriptId);
				if (scriptIndex !== -1) {
					scripts[scriptIndex] = updatedScript;
				}

				const openScriptIndex = openScripts.findIndex((s) => s.id === activeScriptId);
				if (openScriptIndex !== -1) {
					openScripts[openScriptIndex] = updatedScript;
				}

				// Update state management
				scriptContents.delete(activeScriptId);
				scriptContents.set(scriptId, content);
				unsavedChanges.delete(activeScriptId);
				newScripts.delete(activeScriptId);

				// Update active script ID
				activeScriptId = scriptId;
			} else {
				await DatabaseCommands.updateScript(
					activeScriptId,
					currentScript.name,
					content,
					currentScript.connection_id || undefined,
					currentScript.description || undefined
				);

				// Update local state
				currentScript.query_text = content;
				currentScript.updated_at = Date.now() / 1000;
				scriptContents.set(activeScriptId, content);
				unsavedChanges = new Set([...unsavedChanges].filter((id) => id !== activeScriptId));

				// Update scripts list
				const scriptIndex = scripts.findIndex((s) => s.id === activeScriptId);
				if (scriptIndex !== -1) {
					scripts[scriptIndex] = { ...currentScript };
				}
			}
		} catch (error) {
			console.error('Failed to save script:', error);
		}
	}

	async function deleteScript(script: Script) {
		try {
			const isNewScript = newScripts.has(script.id);

			if (!isNewScript) {
				// delete from SQLite only if it was there before
				await DatabaseCommands.deleteScript(script.id);
			}

			// drop from local state
			scripts = scripts.filter((s) => s.id !== script.id);

			// If the script is currently open in a tab, convert it to a new/unsaved script
			const openScriptIndex = openScripts.findIndex((s) => s.id === script.id);
			if (openScriptIndex !== -1) {
				const tempId = nextTempId--;

				const updatedScript = {
					...script,
					id: tempId
				};

				openScripts[openScriptIndex] = updatedScript;

				const currentContent = scriptContents.get(script.id) || script.query_text;
				scriptContents.delete(script.id);
				scriptContents.set(tempId, currentContent);

				if (unsavedChanges.has(script.id)) {
					unsavedChanges.delete(script.id);
					unsavedChanges.add(tempId);
				}

				newScripts.add(tempId);
				if (activeScriptId === script.id) {
					activeScriptId = tempId;
				}

				scripts.push(updatedScript);
			} else {
				// Script not open in tabs - clean up normally
				if (activeScriptId === script.id) {
					activeScriptId = null;
					scriptContents.delete(script.id);
					unsavedChanges = new Set([...unsavedChanges].filter((id) => id !== script.id));
					newScripts.delete(script.id);
				}
			}
		} catch (error) {
			console.error('Failed to delete script:', error);
		}
	}

	async function renameScript(scriptId: number, newName: string) {
		try {
			const script = scripts.find((s) => s.id === scriptId);
			if (!script) return;

			const isNewScript = newScripts.has(scriptId);

			if (!isNewScript) {
				// Only update in SQLite if it was previously saved
				await DatabaseCommands.updateScript(
					scriptId,
					newName,
					script.query_text,
					script.connection_id || undefined,
					script.description || undefined
				);
			}

			// Update the script in the scripts array
			const scriptIndex = scripts.findIndex((s) => s.id === scriptId);
			if (scriptIndex !== -1) {
				scripts[scriptIndex] = {
					...script,
					name: newName,
					updated_at: Date.now() / 1000
				};
			}

			const openScriptIndex = openScripts.findIndex((s) => s.id === scriptId);
			if (openScriptIndex !== -1) {
				openScripts[openScriptIndex] = {
					...openScripts[openScriptIndex],
					name: newName,
					updated_at: Date.now() / 1000
				};
			}
		} catch (error) {
			console.error('Failed to rename script:', error);
		}
	}
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="from-background via-background to-muted/20 flex h-full bg-gradient-to-br">
	<ResizablePaneGroup direction="horizontal" class="flex-1" onLayoutChange={handlePaneResize}>
		<!-- Sidebar Pane - always present but with conditional behavior -->
		<ResizablePane
			defaultSize={isSidebarCollapsed ? 4 : 25}
			minSize={isSidebarCollapsed ? 4 : 8}
			maxSize={isSidebarCollapsed ? 40 : 40}
			class="transition-all duration-300 ease-out"
		>
			<div
				class="bg-sidebar/80 glass-subtle border-sidebar-border relative flex h-full flex-col border-r"
			>
				{#if isSidebarCollapsed}
					<!-- Collapsed Sidebar Content -->
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
								onclick={() => (showConnectionForm = true)}
								title="Add Connection"
							>
								<Plus class="h-4 w-4" />
							</Button>
						</div>
					</div>

					<div class="flex-1 space-y-3 p-3">
						{#each connections as connection}
							<button
								class="flex h-10 w-full items-center justify-center rounded-lg transition-all duration-200 hover:shadow-md {selectedConnection ===
								connection.id
									? 'bg-primary/20 border-primary/30 border shadow-md'
									: 'hover:bg-sidebar-accent/60'}"
								onclick={() => selectConnection(connection.id)}
								title={connection.name}
							>
								<div
									class="h-3 w-3 rounded-full shadow-sm {connection.connected
										? 'bg-success shadow-success/30'
										: establishingConnections.has(connection.id)
											? 'bg-primary shadow-primary/30 animate-pulse'
											: 'bg-muted-foreground/40'}"
								></div>
							</button>
						{/each}
					</div>
				{:else}
					<!-- Expanded Sidebar Content -->
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
							{#snippet children()}
								<!-- Connections Accordion -->
								<AccordionItem title="Connections" icon={Database} open={true}>
									{#snippet children()}
										<AccordionContent>
											{#snippet children()}
												<div class="space-y-3">
													<Button
														class="w-full justify-start gap-2 shadow-sm hover:shadow-md"
														variant="outline"
														onclick={() => (showConnectionForm = true)}
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
																<p class="text-muted-foreground mb-1 text-xs font-medium">
																	No connections yet
																</p>
																<p class="text-muted-foreground/70 text-xs">
																	Add your first connection to get started
																</p>
															</div>
														{:else}
															{#each connections as connection (connection.id)}
																<Button
																	variant={selectedConnection === connection.id
																		? 'secondary'
																		: 'ghost'}
																	class="h-auto w-full justify-start p-3 shadow-sm transition-all duration-200 hover:shadow-md {selectedConnection ===
																	connection.id
																		? 'bg-primary/10 border-primary/20 border shadow-md'
																		: ''}"
																	onclick={() => selectConnection(connection.id)}
																	ondblclick={() => connectToDatabase(connection.id)}
																>
																	<div class="flex w-full items-start gap-3">
																		<div class="mt-1 flex-shrink-0">
																			{#if connection.connected}
																				<div
																					class="bg-success border-success-light h-2.5 w-2.5 rounded-full border shadow-sm"
																				></div>
																			{:else if establishingConnections.has(connection.id)}
																				<div
																					class="bg-primary border-primary-light h-2.5 w-2.5 animate-pulse rounded-full border shadow-sm"
																				></div>
																			{:else}
																				<div
																					class="bg-muted-foreground/40 border-muted-foreground/20 h-2.5 w-2.5 rounded-full border"
																				></div>
																			{/if}
																		</div>

																		<div class="min-w-0 flex-1 text-left">
																			<div
																				class="text-foreground mb-1 truncate text-xs font-medium"
																			>
																				{connection.name}
																			</div>
																			<div class="text-muted-foreground/80 truncate text-xs">
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
											{/snippet}
										</AccordionContent>
									{/snippet}
								</AccordionItem>

								<!-- Scripts Accordion -->
								<AccordionItem title="Scripts" icon={FileText} open={false}>
									{#snippet children()}
										<AccordionContent>
											{#snippet children()}
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
																				<div
																					class="text-foreground mb-1 truncate text-xs font-medium"
																				>
																					{script.name}
																					{#if activeScriptId === script.id && unsavedChanges.has(script.id)}
																						<span class="text-orange-500">*</span>
																					{/if}
																				</div>
																				<div class="text-muted-foreground/80 truncate text-xs">
																					Modified {new Date(
																						script.updated_at * 1000
																					).toLocaleDateString()}
																				</div>
																			</div>
																		</div>
																	</Button>

																	<!-- Delete button (visible on hover) -->
																	<button
																		class="absolute top-2 right-2 rounded p-1 opacity-0 transition-all group-hover:opacity-100 hover:bg-red-100 hover:text-red-600"
																		onclick={(e) => {
																			e.stopPropagation();
																			deleteScript(script);
																		}}
																		title="Delete script"
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
											{/snippet}
										</AccordionContent>
									{/snippet}
								</AccordionItem>

								<!-- Items Accordion -->
								<AccordionItem title="Items" icon={Table} bind:open={isItemsAccordionOpen}>
									{#snippet children()}
										<AccordionContent>
											{#snippet children()}
												{#if isItemsAccordionOpen}
													<DatabaseSchemaItems
														{databaseSchema}
														{loadingSchema}
														{selectedConnection}
														onTableClick={handleTableClick}
													/>
												{/if}
											{/snippet}
										</AccordionContent>
									{/snippet}
								</AccordionItem>
							{/snippet}
						</Accordion>
					</div>
				{/if}
			</div>
		</ResizablePane>

		<ResizableHandle />

		<!-- Main Editor Pane - always in same position -->
		<ResizablePane defaultSize={isSidebarCollapsed ? 96 : 75}>
			<div class="bg-background/50 flex h-full flex-col">
				<!-- Toolbar -->
				<div class="glass-card border-border/50 border-b p-6 shadow-md">
					<div class="flex items-center justify-between">
						<div class="flex items-center gap-4">
							<div class="flex items-center gap-3">
								<Button
									class="gap-2 shadow-md hover:shadow-lg"
									disabled={!selectedConnection}
									onclick={() => sqlEditorRef?.handleExecuteQuery()}
									title="Run Query (Ctrl+R for full script, Ctrl+Enter for selection)"
								>
									<Play class="h-4 w-4" />
									Run Query
								</Button>
								<Button
									variant="outline"
									class="gap-2 shadow-sm hover:shadow-md {activeScriptId !== null &&
									unsavedChanges.has(activeScriptId)
										? 'border-orange-300 bg-orange-50'
										: ''}"
									onclick={saveCurrentScript}
								>
									<Save class="h-4 w-4" />
									Save Script {activeScriptId !== null && unsavedChanges.has(activeScriptId)
										? '*'
										: ''}
								</Button>
							</div>

							{#if selectedConnection}
								{@const connection = connections.find((c) => c.id === selectedConnection)}
								{#if connection}
									{#if connection.connected}
										<div
											class="bg-success-light/20 border-success/30 flex items-center gap-2 rounded-lg border px-4 py-2"
										>
											<div class="bg-success h-2 w-2 rounded-full shadow-sm"></div>
											<span class="text-foreground text-sm font-semibold"
												>Connected to: {connection.name}</span
											>
										</div>
									{:else if establishingConnections.has(connection.id)}
										<div
											class="bg-primary/10 border-primary/30 flex items-center gap-2 rounded-lg border px-4 py-2"
										>
											<div class="bg-primary h-2 w-2 animate-pulse rounded-full shadow-sm"></div>
											<span class="text-foreground text-sm font-semibold"
												>Connecting to: {connection.name}</span
											>
										</div>
									{:else}
										<div
											class="bg-muted/20 border-border flex items-center gap-2 rounded-lg border px-4 py-2"
										>
											<div class="bg-muted-foreground/60 h-2 w-2 rounded-full shadow-sm"></div>
											<span class="text-foreground text-sm font-semibold"
												>Selected: {connection.name} (double-click to connect)</span
											>
										</div>
									{/if}
								{/if}
							{:else}
								<div
									class="bg-muted/30 border-border flex items-center gap-2 rounded-lg border px-4 py-2"
								>
									<span class="text-muted-foreground text-sm font-semibold"
										>Select a connection to start</span
									>
								</div>
							{/if}
						</div>
					</div>
				</div>

				<!-- Editor and Results - same component instance always -->
				<div class="bg-background/30 flex flex-1 flex-col">
					<!-- Script Tabs -->
					<ScriptTabs
						{openScripts}
						{activeScriptId}
						{unsavedChanges}
						onTabSelect={switchToTab}
						onTabClose={closeTab}
						onNewScript={createNewScript}
						onScriptRename={renameScript}
					/>

					<SqlEditor
						{selectedConnection}
						{connections}
						currentScript={activeScriptId !== null
							? scripts.find((s) => s.id === activeScriptId) || null
							: null}
						hasUnsavedChanges={activeScriptId !== null && unsavedChanges.has(activeScriptId)}
						bind:this={sqlEditorRef}
						onContentChange={handleEditorContentChange}
						onLoadFromHistory={loadQueryFromHistory}
					/>
				</div>
			</div>
		</ResizablePane>
	</ResizablePaneGroup>
</div>

<!-- Table Browse Modal -->
<TableBrowseModal
	isOpen={tableBrowseModalOpen}
	tableName={selectedTableName}
	schema={selectedTableSchema}
	connectionId={selectedConnection || ''}
	onClose={closeTableBrowseModal}
/>

<!-- Connection Form Modal -->
{#if showConnectionForm}
	<div
		class="bg-background/80 animate-fade-in fixed inset-0 z-50 flex items-center justify-center backdrop-blur-sm"
	>
		<div
			class="glass-card hover-lift mx-4 max-h-[90vh] w-full max-w-lg overflow-y-auto rounded-xl p-8 shadow-xl"
		>
			<ConnectionForm onSubmit={addConnection} onCancel={() => (showConnectionForm = false)} />
		</div>
	</div>
{/if}
