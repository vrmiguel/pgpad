<script lang="ts">
	import { ResizablePaneGroup, ResizablePane, ResizableHandle } from '$lib/components/ui/resizable';
	import SqlEditor from './SqlEditor.svelte';
	import TableBrowseView from './TableBrowseView.svelte';
	import ConnectionForm from './ConnectionForm.svelte';
	import ScriptTabs from './ScriptTabs.svelte';
	import AppSidebar from './AppSidebar.svelte';
	import {
		Commands,
		type ConnectionInfo,
		type DatabaseInfo,
		type Script,
		type DatabaseSchema,
		type QueryHistoryEntry
	} from '$lib/commands.svelte';
	import { onMount, onDestroy } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import { SvelteSet } from 'svelte/reactivity';
	import { tabs, type ScriptTab, type SidebarTabState } from '$lib/stores/tabs.svelte';

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

	const scripts = $derived(tabs.scripts);
	const openScripts = $derived(
		tabs.all.filter((tab) => tab.type === 'script').map((tab) => (tab as ScriptTab).script)
	);
	const activeScriptId = $derived(
		tabs.active?.type === 'script' ? (tabs.active as ScriptTab).scriptId : null
	);
	const currentEditorContent = $derived(tabs.currentEditorContent);

	let isSidebarCollapsed = $state(false);
	let lastResizeTime = $state(0);

	let databaseSchema = $state<DatabaseSchema | null>(null);
	let loadingSchema = $state(false);
	let queryHistory = $state<QueryHistoryEntry[]>([]);
	let lastLoadedSchemaConnectionId = $state<string | null>(null);

	let unlistenDisconnect: (() => void) | null = null;

	if (selectedConnection === undefined) {
		selectedConnection = null;
	}

	$effect(() => {
		if (hasUnsavedChanges !== undefined) {
			hasUnsavedChanges = tabs.active?.isDirty ?? false;
		}
	});

	if (runQueryCallback !== undefined) {
		runQueryCallback = () => sqlEditorRef?.handleExecuteQuery();
	}
	if (saveScriptCallback !== undefined) {
		saveScriptCallback = saveCurrentScript;
	}
	let sidebarTabState: SidebarTabState = $state('connections');

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
			await tabs.saveSession(async (tabSessionData) => {
				const fullSessionData = {
					...tabSessionData,
					selectedConnection,
					isSidebarCollapsed,
					sidebarTabState
				};
				await Commands.saveSessionState(JSON.stringify(fullSessionData));
			});
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
			if (saved.sidebarTabState !== undefined) sidebarTabState = saved.sidebarTabState;

			const restored = await tabs.restoreSession(saved);
			return restored;
		} catch (e) {
			console.error('Failed to restore session:', e);
			return false;
		}
	}

	// Auto-collapse if resized below 20%
	const COLLAPSE_THRESHOLD = 20;
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

	$effect(() => {
		if (sqlEditorRef) {
			tabs.setSqlEditorRef(sqlEditorRef);
		}
	});

	$effect(() => {
		if (sqlEditorRef && currentEditorContent !== undefined) {
			const currentContent = sqlEditorRef.getContent();
			if (currentContent !== currentEditorContent) {
				sqlEditorRef.setContentSilently(currentEditorContent);
			}
		}
	});

	function handleEditorContentChange(newContent: string) {
		tabs.handleEditorContentChange(newContent);
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
		tabs.openScript(script);
		markSessionDirty();
	}

	function selectScript(script: Script) {
		openScript(script);
	}

	async function createScriptFromHistory(historyQuery: string) {
		tabs.createScriptFromHistory(historyQuery);
		markSessionDirty();
	}

	onMount(async () => {
		try {
			await Commands.initializeConnections();
			await loadConnections();
			await loadScripts();

			tabs.setScripts(scripts);

			tabs.onSessionSave(() => markSessionDirty());

			unlistenDisconnect = await listen('end-of-connection', (event) => {
				const connectionId = event.payload as string;
				handleConnectionDisconnect(connectionId);
			});

			// checks if we should auto-save the session, every 20 secs
			sessionSaveTimer = setInterval(() => {
				checkAndSaveSession().catch(console.error);
			}, 20000);

			const restored = await restoreSession();
			// If we restored a session and still no scripts were loaded
			if (!restored && tabs.all.length === 0) {
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

	async function handleConnectionSubmit(name: string, databaseInfo: DatabaseInfo) {
		try {
			if (editingConnection) {
				const updated = await Commands.updateConnection(editingConnection.id, name, databaseInfo);
				const i = connections.findIndex((c) => c.id === editingConnection!.id);
				if (i !== -1) connections[i] = updated;
			} else {
				const created = await Commands.addConnection(name, databaseInfo);
				connections.push(created);
			}
			showConnectionForm = false;
			editingConnection = null;
		} catch (error) {
			console.error('Failed to save connection:', error);
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
				await loadConnections();
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
		if (!selectedConnection) return;
		tabs.openTableExplorationTab(tableName, schema, selectedConnection);
		markSessionDirty();
	}

	async function loadQueryHistory() {
		if (!selectedConnection) {
			queryHistory = [];
			return;
		}

		try {
			queryHistory = await Commands.getQueryHistory(selectedConnection, 50);
		} catch (error) {
			console.error('Failed to load query history:', error);
			queryHistory = [];
		}
	}

	function loadQueryFromHistory(historyQuery: string) {
		sqlEditorRef?.loadQueryFromHistory(historyQuery);
	}

	$effect(() => {
		if (selectedConnection) {
			loadQueryHistory();
		}
	});

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
			const loadedScripts = await Commands.getScripts();
			tabs.setScripts(loadedScripts);
		} catch (error) {
			console.error('Failed to load scripts:', error);
		}
	}

	async function createNewScript() {
		try {
			tabs.createNewScript();
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

			const isNewScript = tabs.newScripts.has(activeScriptId);

			if (isNewScript) {
				const scriptId = await Commands.saveScript(
					currentScript.name,
					content,
					currentScript.connection_id || undefined,
					currentScript.description || undefined
				);

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

				tabs.newScripts.delete(activeScriptId);

				// Update tab with new database ID
				tabs.updateScriptId(activeScriptId, scriptId, updatedScript);

				tabs.markScriptSaved(scriptId, content);
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

				// Update scripts list
				const scriptIndex = scripts.findIndex((s) => s.id === activeScriptId);
				if (scriptIndex !== -1) {
					scripts[scriptIndex] = { ...currentScript };
				}

				// Notify tab store that script is saved
				tabs.markScriptSaved(activeScriptId, content);
			}
			markSessionDirty();
		} catch (error) {
			console.error('Failed to save script:', error);
		}
	}

	async function deleteScript(script: Script) {
		try {
			const tabId = `script-${script.id}`;
			const tab = tabs.all.find((t) => t.id === tabId);
			const isNewScript =
				tab?.type === 'script' ? (tab as ScriptTab).isNewScript : tabs.newScripts.has(script.id);

			if (!isNewScript) {
				// delete from SQLite only if it was there before
				await Commands.deleteScript(script.id);
			}

			// Remove from scripts array
			const scriptIndex = tabs.scripts.findIndex((s) => s.id === script.id);
			if (scriptIndex !== -1) {
				tabs.scripts.splice(scriptIndex, 1);
			}

			// If the script is currently open in a tab, convert it to a new/unsaved script
			if (tab?.type === 'script') {
				const tempId = tabs.nextTempId;
				tabs.nextTempId--;

				const updatedScript = {
					...script,
					id: tempId
				};

				const scriptTab = tab as ScriptTab;
				scriptTab.scriptId = tempId;
				scriptTab.script = updatedScript;
				scriptTab.isNewScript = true;
				scriptTab.id = `script-${tempId}`;

				tabs.newScripts.add(tempId);
				tabs.scripts.push(updatedScript);

				// Update active tab ID if this was the active tab
				if (tabs.activeId === tabId) {
					tabs.switchToTab(scriptTab.id);
				}
			}

			markSessionDirty();
		} catch (error) {
			console.error('Failed to delete script:', error);
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
				unsavedChanges={new SvelteSet(
					tabs.all
						.filter((t) => t.isDirty)
						.map((t) => (t.type === 'script' ? (t as ScriptTab).scriptId : 0))
				)}
				{databaseSchema}
				{loadingSchema}
				{queryHistory}
				bind:isSidebarCollapsed
				bind:sidebarTabState
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
				onLoadFromHistory={loadQueryFromHistory}
			/>
		</ResizablePane>

		<ResizableHandle />

		<!-- Main Editor Pane -->
		<ResizablePane defaultSize={isSidebarCollapsed ? 96 : 75}>
			<div class="flex h-full flex-col bg-white dark:bg-gray-900">
				<!-- Editor and Results - conditional rendering based on tab type -->
				<div class="flex flex-1 flex-col bg-gray-50/50 dark:bg-gray-800/50">
					<!-- Script Tabs -->
					<ScriptTabs />

					{#if tabs.active?.type === 'script'}
						<SqlEditor
							selectedConnection={selectedConnection ?? null}
							{connections}
							currentScript={activeScriptId !== null
								? scripts.find((s) => s.id === activeScriptId) || null
								: null}
							hasUnsavedChanges={tabs.active?.isDirty ?? false}
							bind:this={sqlEditorRef}
							onContentChange={handleEditorContentChange}
							onLoadFromHistory={createScriptFromHistory}
							onHistoryUpdate={loadQueryHistory}
						/>
					{:else if tabs.active?.type === 'table-view'}
						{@const tableTab = tabs.active}
						<TableBrowseView
							tableName={tableTab.tableName}
							schema={tableTab.schema}
							connectionId={tableTab.connectionId}
						/>
					{/if}
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
