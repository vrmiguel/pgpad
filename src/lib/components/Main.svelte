<script lang="ts">
	import { ResizablePaneGroup, ResizablePane, ResizableHandle } from '$lib/components/ui/resizable';
	import SqlEditor from './SqlEditor.svelte';
	import ConnectionForm from './ConnectionForm.svelte';
	import ScriptTabs from './ScriptTabs.svelte';
	import AppSidebar from './AppSidebar.svelte';
	import {
		Commands,
		type ConnectionInfo,
		type ConnectionConfig,
		type Script,
		type DatabaseSchema
	} from '$lib/commands.svelte';
	import { onMount, onDestroy } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import { SvelteSet } from 'svelte/reactivity';

	interface Props {
		currentConnection?: {
			name: string;
			connected: boolean;
		} | null;
		isConnecting?: boolean;
		selectedConnection?: string | null;
		hasUnsavedChanges?: boolean;
		runQueryCallback?: (() => void) | null;
		saveScriptCallback?: (() => void) | null;
	}

	let {
		currentConnection = $bindable(),
		isConnecting = $bindable(),
		selectedConnection = $bindable(),
		hasUnsavedChanges = $bindable(),
		runQueryCallback = $bindable(),
		saveScriptCallback = $bindable()
	}: Props = $props();

	let showConnectionForm = $state(false);
	let editingConnection = $state<ConnectionInfo | null>(null);
	let connections = $state<ConnectionInfo[]>([]);
	let sqlEditorRef = $state<SqlEditor>();
	let establishingConnections = new SvelteSet<string>();

	let scripts = $state<Script[]>([]);
	let openScripts = $state<Script[]>([]);
	let activeScriptId = $state<number | null>(null);
	let unsavedChanges = new SvelteSet<number>();
	let scriptContents = $state<Map<number, string>>(new Map());
	let currentEditorContent = $state<string>('');
	// Scripts not yet persisted in SQLite
	let newScripts = new SvelteSet<number>();
	// Let's use negative IDs for unpersisted scripts
	let nextTempId = $state(-1);

	let isSidebarCollapsed = $state(false);
	let lastResizeTime = $state(0);

	let databaseSchema = $state<DatabaseSchema | null>(null);
	let loadingSchema = $state(false);
	let lastLoadedSchemaConnectionId = $state<string | null>(null);

	let isItemsAccordionOpen = $state(false);

	let unlistenDisconnect: (() => void) | null = null;

	if (selectedConnection === undefined) {
		selectedConnection = null;
	}

	$effect(() => {
		if (hasUnsavedChanges !== undefined) {
			hasUnsavedChanges = activeScriptId !== null && unsavedChanges.has(activeScriptId);
		}
	});

	if (runQueryCallback !== undefined) {
		runQueryCallback = () => sqlEditorRef?.handleExecuteQuery();
	}
	if (saveScriptCallback !== undefined) {
		saveScriptCallback = saveCurrentScript;
	}
	let isConnectionsAccordionOpen = $state(true);
	let isScriptsAccordionOpen = $state(false);

	// TODO(vini): turn into an $effect that updates with its dependencies
	let shouldSaveSession = false;
	let sessionSaveTimer: ReturnType<typeof setInterval> | null = null;

	function markSessionDirty() {
		shouldSaveSession = true;
	}

	async function checkAndSaveSession() {
		if (shouldSaveSession) {
			shouldSaveSession = false;
			try {
				await saveSessionNow();
			} catch (e) {
				shouldSaveSession = true;
				console.error('Failed to save session:', e);
			}
		}
	}

	async function saveSessionNow() {
		try {
			await Commands.saveSessionState(
				JSON.stringify({
					selectedConnection,
					isSidebarCollapsed,
					isConnectionsAccordionOpen,
					isScriptsAccordionOpen,
					isItemsAccordionOpen,
					openScriptIds: openScripts.map((s) => s.id),
					activeScriptId,
					unsavedChanges: Object.fromEntries(
						Array.from(scriptContents.entries()).filter(([id, content]) => {
							if (newScripts.has(id)) return false;
							const original = scripts.find((s) => s.id === id);
							return original && content !== original.query_text;
						})
					),
					tempScripts: Array.from(newScripts)
						.map((id) => {
							const script = scripts.find((s) => s.id === id);
							return script
								? {
										id: script.id,
										name: script.name,
										content: scriptContents.get(id) ?? script.query_text
									}
								: null;
						})
						.filter(Boolean),
					nextTempId
				})
			);
		} catch (e) {
			console.error('Failed to save session:', e);
		}
	}

	async function restoreSession(): Promise<boolean> {
		try {
			const raw = await Commands.getSessionState();
			if (!raw) return false;

			const saved = JSON.parse(raw);

			if (saved.selectedConnection !== undefined) selectedConnection = saved.selectedConnection;
			if (saved.isSidebarCollapsed !== undefined) isSidebarCollapsed = saved.isSidebarCollapsed;
			if (saved.isConnectionsAccordionOpen !== undefined)
				isConnectionsAccordionOpen = saved.isConnectionsAccordionOpen;
			if (saved.isScriptsAccordionOpen !== undefined)
				isScriptsAccordionOpen = saved.isScriptsAccordionOpen;
			if (saved.isItemsAccordionOpen !== undefined)
				isItemsAccordionOpen = saved.isItemsAccordionOpen;
			if (saved.nextTempId !== undefined) nextTempId = Math.min(nextTempId, saved.nextTempId);

			for (const temp of saved.tempScripts ?? []) {
				if (!scripts.find((s) => s.id === temp.id)) {
					scripts.push({
						id: temp.id,
						name: temp.name,
						description: null,
						query_text: temp.content,
						connection_id: null,
						tags: null,
						created_at: Date.now() / 1000,
						updated_at: Date.now() / 1000,
						favorite: false
					});
				}
				newScripts.add(temp.id);
				scriptContents.set(temp.id, temp.content);
			}

			for (const [idStr, content] of Object.entries(saved.unsavedChanges ?? {})) {
				scriptContents.set(parseInt(idStr), content as string);
			}

			for (const id of saved.openScriptIds ?? []) {
				const script = scripts.find((s) => s.id === id);
				if (script) openScript(script);
			}

			if (saved.activeScriptId != null) {
				const script = scripts.find((s) => s.id === saved.activeScriptId);
				if (script) switchToTab(script.id);
			}

			return (saved.openScriptIds?.length ?? 0) > 0;
		} catch (e) {
			console.error('Failed to restore session:', e);
			return false;
		}
	}

	// Auto-collapse if resized below 12%
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
					unsavedChanges.add(activeScriptId);
				}
			} else {
				if (unsavedChanges.has(activeScriptId)) {
					unsavedChanges.delete(activeScriptId);
				}
			}
		}
	});

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
				markSessionDirty();
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

		markSessionDirty();
	}

	function switchToTab(scriptId: number) {
		// Save current script content before switching
		if (activeScriptId !== null) {
			scriptContents.set(activeScriptId, currentEditorContent);
		}

		activeScriptId = scriptId;
		const script = scripts.find((s) => s.id === scriptId);
		if (script) {
			const content = scriptContents.get(scriptId) || script.query_text;
			if (sqlEditorRef) {
				sqlEditorRef.setContent(content);
			}
		}
		markSessionDirty();
	}

	function closeTab(scriptId: number) {
		openScripts = openScripts.filter((s) => s.id !== scriptId);

		scriptContents.delete(scriptId);
		unsavedChanges.delete(scriptId);
		newScripts.delete(scriptId);

		// if closing the active tab, switch to another or clear active
		if (activeScriptId === scriptId) {
			if (openScripts.length > 0) {
				// Switch to the last remaining tab
				const lastScript = openScripts[openScripts.length - 1];
				switchToTab(lastScript.id);
			} else {
				// no more open tabs left
				activeScriptId = null;
				if (sqlEditorRef) {
					sqlEditorRef.setContent('');
				}
			}
		}
		markSessionDirty();
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
			await Commands.initializeConnections();
			await loadConnections();
			await loadScripts();

			unlistenDisconnect = await listen('end-of-connection', (event) => {
				const connectionId = event.payload as string;
				handleConnectionDisconnect(connectionId);
			});

			// checks if we should auto-save the sesssion, every 20 secs
			sessionSaveTimer = setInterval(() => {
				checkAndSaveSession().catch(console.error);
			}, 20000);

			const restored = await restoreSession();
			// If we restored a session and still no scripts were loaded
			if (!restored && openScripts.length === 0) {
				const existingUntitledScript = scripts.find((s) => s.name === 'Untitled Script');
				if (existingUntitledScript) {
					openScript(existingUntitledScript);
				} else {
					await createNewScript();
				}
			}
		} catch (error) {
			console.error(`Failed to initialize connections: ${JSON.stringify(error)}`);
		}
	});

	onDestroy(() => {
		if (unlistenDisconnect) {
			unlistenDisconnect();
		}

		if (sessionSaveTimer) {
			clearInterval(sessionSaveTimer);
		}

		checkAndSaveSession().catch(console.error);
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
			connections = await Commands.getConnections();
		} catch (error) {
			console.error('Failed to load connections:', error);
		}
	}

	function handleConnectionDisconnect(connectionId: string) {
		console.log('Connection disconnected:', connectionId);

		connections = connections.map((conn) =>
			conn.id === connectionId ? { ...conn, connected: false } : conn
		);

		if (selectedConnection === connectionId) {
			databaseSchema = null;
			lastLoadedSchemaConnectionId = null;
		}

		establishingConnections.delete(connectionId);
	}

	// receives a ConnectionConfig from <ConnectionForm />
	async function handleConnectionSubmit(config: ConnectionConfig) {
		try {
			if (editingConnection) {
				const updated = await Commands.updateConnection(editingConnection.id, config);
				const i = connections.findIndex((c) => c.id === editingConnection!.id);
				if (i !== -1) connections[i] = updated;
			} else {
				const created = await Commands.addConnection(config);
				connections.push(created);
			}
			showConnectionForm = false;
			editingConnection = null;
		} catch (error) {
			console.error('Failed to save connection:', error);
		}
	}

	async function addConnection(config: ConnectionConfig) {
		try {
			const newConnection = await Commands.addConnection(config);
			connections.push(newConnection);
			showConnectionForm = false;
		} catch (error) {
			console.error('Failed to add connection:', error);
		}
	}

	function selectConnection(connectionId: string) {
		selectedConnection = connectionId;
		markSessionDirty();
	}

	async function connectToDatabase(connectionId: string) {
		establishingConnections.add(connectionId);

		try {
			const success = await Commands.connectToDatabase(connectionId);
			if (success) {
				// Update the connection status
				await loadConnections();
				// Load schema after successful connection
				if (selectedConnection === connectionId) {
					await loadDatabaseSchema();
				}
			}
		} catch (error) {
			console.error('Failed to connect:', error);
		} finally {
			establishingConnections.delete(connectionId);
		}
	}

	async function loadDatabaseSchemaIfNeeded(connectionId: string) {
		// Don't reload if already loaded for this connection or currently loading
		if (lastLoadedSchemaConnectionId === connectionId || loadingSchema) {
			return;
		}

		try {
			loadingSchema = true;
			databaseSchema = await Commands.getDatabaseSchema(connectionId);
			lastLoadedSchemaConnectionId = connectionId;
		} catch (error) {
			console.error('Failed to load database schema:', error);
			databaseSchema = null;
			lastLoadedSchemaConnectionId = null;
		} finally {
			loadingSchema = false;
		}
	}

	async function loadDatabaseSchema() {
		if (!selectedConnection || loadingSchema) return;

		try {
			loadingSchema = true;
			// Reset the last loaded ID to force reload
			lastLoadedSchemaConnectionId = null;
			databaseSchema = await Commands.getDatabaseSchema(selectedConnection);
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
		sqlEditorRef?.handleTableBrowse(tableName, schema);
	}

	function editConnection(connection: ConnectionInfo) {
		editingConnection = connection;
		showConnectionForm = true;
	}

	async function deleteConnection(connectionId: string) {
		try {
			await Commands.removeConnection(connectionId);
			await loadConnections();
			// If the deleted connection was selected, clear the selection
			if (selectedConnection === connectionId) {
				selectedConnection = null;
			}
		} catch (error) {
			console.error('Failed to delete connection:', error);
		}
	}

	async function disconnectConnection(connectionId: string) {
		try {
			await Commands.disconnectFromDatabase(connectionId);
			await loadConnections();
		} catch (error) {
			console.error('Failed to disconnect:', error);
		}
	}

	async function loadScripts() {
		try {
			scripts = await Commands.getScripts();
		} catch (error) {
			console.error('Failed to load scripts:', error);
		}
	}

	function generateScriptName(): string {
		const existingUntitled = scripts.filter((s) => s.name.startsWith('Untitled Script')).length;
		return existingUntitled === 0 ? 'Untitled Script' : `Untitled Script ${existingUntitled + 1}`;
	}

	async function createNewScript() {
		try {
			const name = generateScriptName();
			const content = `-- Welcome to PgPad!
-- Keyboard shortcuts:
--   Ctrl+Enter: Run selected text (or current line if nothing selected)
--   Ctrl+R: Run entire script

SELECT 1 as test;`;
			// New scripts get negative IDs
			const tempId = nextTempId--;

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
			markSessionDirty();
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
				const scriptId = await Commands.saveScript(
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
				await Commands.updateScript(
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
				unsavedChanges.delete(activeScriptId);

				// Update scripts list
				const scriptIndex = scripts.findIndex((s) => s.id === activeScriptId);
				if (scriptIndex !== -1) {
					scripts[scriptIndex] = { ...currentScript };
				}
			}
			markSessionDirty();
		} catch (error) {
			console.error('Failed to save script:', error);
		}
	}

	async function deleteScript(script: Script) {
		try {
			const isNewScript = newScripts.has(script.id);

			if (!isNewScript) {
				// delete from SQLite only if it was there before
				await Commands.deleteScript(script.id);
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
					unsavedChanges.delete(script.id);
					newScripts.delete(script.id);
				}
			}
			markSessionDirty();
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
				await Commands.updateScript(
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
			markSessionDirty();
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
			<AppSidebar
				{connections}
				selectedConnection={selectedConnection ?? null}
				{establishingConnections}
				{scripts}
				{activeScriptId}
				{unsavedChanges}
				{databaseSchema}
				{loadingSchema}
				bind:isSidebarCollapsed
				bind:isConnectionsAccordionOpen
				bind:isScriptsAccordionOpen
				bind:isItemsAccordionOpen
				onSelectConnection={selectConnection}
				onConnectToDatabase={connectToDatabase}
				onShowConnectionForm={() => {
					editingConnection = null;
					showConnectionForm = true;
				}}
				onEditConnection={editConnection}
				onDeleteConnection={deleteConnection}
				onDisconnectConnection={disconnectConnection}
				onSelectScript={selectScript}
				onCreateNewScript={createNewScript}
				onDeleteScript={deleteScript}
				onTableClick={handleTableClick}
			/>
		</ResizablePane>

		<ResizableHandle />

		<!-- Main Editor Pane -->
		<ResizablePane defaultSize={isSidebarCollapsed ? 96 : 75}>
			<div class="flex h-full flex-col bg-white dark:bg-gray-900">
				<!-- Editor and Results - same component instance always -->
				<div class="flex flex-1 flex-col bg-gray-50/50 dark:bg-gray-800/50">
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
						selectedConnection={selectedConnection ?? null}
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

{#if showConnectionForm}
	<div
		class="bg-background/80 animate-fade-in fixed inset-0 z-50 flex items-center justify-center backdrop-blur-sm"
	>
		<div
			class="glass-card hover-lift mx-4 max-h-[90vh] w-full max-w-2xl overflow-y-auto rounded-xl p-6 shadow-xl"
		>
			<ConnectionForm
				onSubmit={handleConnectionSubmit}
				onCancel={() => {
					showConnectionForm = false;
					editingConnection = null;
				}}
				{editingConnection}
			/>
		</div>
	</div>
{/if}
