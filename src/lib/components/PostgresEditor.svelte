<script lang="ts">
	import { Database, Plus, Play, Save, ChevronLeft, ChevronRight } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import { ResizablePaneGroup, ResizablePane, ResizableHandle } from '$lib/components/ui/resizable';
	import ConnectionSidebar from './ConnectionSidebar.svelte';
	import SqlEditor from './SqlEditor.svelte';
	import ConnectionForm from './ConnectionForm.svelte';
	import { DatabaseCommands, type ConnectionInfo, type ConnectionConfig } from '$lib/commands.svelte';
	import { onMount } from 'svelte';

	let showConnectionForm = $state(false);
	let selectedConnection = $state<string | null>(null);
	let connections = $state<ConnectionInfo[]>([]);
	let isRunningQuery = $state(false);
	let sqlEditorRef = $state<any>();
	let establishingConnections = $state<Set<string>>(new Set());
	
	let isSidebarCollapsed = $state(false);

	onMount(async () => {
		try {
			await DatabaseCommands.initializeConnections();
			await loadConnections();
		} catch (error) {
			console.error('Failed to initialize connections:', error);
		}
	});

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
			}
		} catch (error) {
			console.error('Failed to connect:', error);
		} finally {
			establishingConnections = new Set([...establishingConnections].filter(id => id !== connectionId));
		}
	}

	// Sidebar toggle functionality
	function toggleSidebar() {
		isSidebarCollapsed = !isSidebarCollapsed;
	}
</script>

<div class="flex h-screen bg-gray-50">
	{#if isSidebarCollapsed}
		<!-- Collapsed Sidebar -->
		<div class="w-12 bg-white border-r border-gray-200 flex flex-col relative transition-all duration-200">
			<!-- Collapse/Expand Button -->
			<button
				class="absolute top-4 right-2 z-10 p-1 rounded hover:bg-gray-100 transition-colors"
				onclick={toggleSidebar}
				title="Expand sidebar"
			>
				<ChevronRight class="w-4 h-4 text-gray-600" />
			</button>

			<!-- Collapsed sidebar content -->
			<div class="p-2 border-b border-gray-200">
				<div class="flex flex-col items-center gap-2">
					<Database class="w-6 h-6 text-blue-600" />
					<Button
						size="sm"
						variant="outline"
						class="w-8 h-8 p-0"
						onclick={() => showConnectionForm = true}
						title="Add Connection"
					>
						<Plus class="w-4 h-4" />
					</Button>
				</div>
			</div>
			
			<!-- Collapsed connections indicators -->
			<div class="flex-1 p-2 space-y-2">
				{#each connections as connection}
					<button
						class="w-full h-8 rounded flex items-center justify-center transition-colors {selectedConnection === connection.id ? 'bg-blue-100 border border-blue-300' : 'hover:bg-gray-100'}"
						onclick={() => selectConnection(connection.id)}
						title={connection.name}
					>
						<div class="w-2 h-2 rounded-full {connection.connected ? 'bg-green-500' : 'bg-gray-400'}"></div>
					</button>
				{/each}
			</div>
		</div>

		<!-- Main Editor Area (Full Width) -->
		<div class="flex-1 flex flex-col">
			<!-- Toolbar -->
			<div class="bg-white border-b border-gray-200 p-4">
				<div class="flex items-center gap-2">
					<Button class="gap-2" disabled={!selectedConnection} onclick={() => sqlEditorRef?.handleExecuteQuery()}>
						<Play class="w-4 h-4" />
						Run Query
					</Button>
					<Button variant="outline" class="gap-2">
						<Save class="w-4 h-4" />
						Save Script
					</Button>
					
					{#if selectedConnection}
						{@const connection = connections.find(c => c.id === selectedConnection)}
						<div class="ml-auto flex items-center gap-2 text-sm text-gray-600">
							<div class="w-2 h-2 rounded-full bg-green-500"></div>
							Connected to: {connection?.name}
						</div>
					{:else}
						<div class="ml-auto text-sm text-gray-500">
							Select a connection to start
						</div>
					{/if}
				</div>
			</div>

			<!-- Editor and Results -->
			<div class="flex-1 flex flex-col">
				<SqlEditor {selectedConnection} {connections} bind:this={sqlEditorRef} />
			</div>
		</div>
	{:else}
		<!-- Resizable Layout -->
		<ResizablePaneGroup direction="horizontal" class="flex-1">
			<!-- Sidebar Pane -->
			<ResizablePane defaultSize={25} minSize={20} maxSize={40}>
				<div class="h-full bg-white border-r border-gray-200 flex flex-col relative">
					<!-- Collapse/Expand Button -->
					<button
						class="absolute top-4 right-2 z-10 p-1 rounded hover:bg-gray-100 transition-colors"
						onclick={toggleSidebar}
						title="Collapse sidebar"
					>
						<ChevronLeft class="w-4 h-4 text-gray-600" />
					</button>

					<!-- Header -->
					<div class="p-4 border-b border-gray-200">
						<div class="flex items-center gap-2 mb-4">
							<Database class="w-6 h-6 text-blue-600" />
							<h1 class="text-xl font-semibold text-gray-900">PgPad</h1>
						</div>
						
						<Button
							class="w-full justify-start gap-2"
							variant="outline"
							onclick={() => showConnectionForm = true}
						>
							<Plus class="w-4 h-4" />
							Add Connection
						</Button>
					</div>

					<!-- Connections List -->
											<ConnectionSidebar
							{connections}
							{selectedConnection}
							{establishingConnections}
							onSelect={selectConnection}
							onConnect={connectToDatabase}
						/>
				</div>
			</ResizablePane>

			<ResizableHandle />

			<!-- Main Editor Pane -->
			<ResizablePane defaultSize={75}>
				<div class="flex flex-col h-full">
					<!-- Toolbar -->
					<div class="bg-white border-b border-gray-200 p-4">
						<div class="flex items-center gap-2">
							<Button class="gap-2" disabled={!selectedConnection} onclick={() => sqlEditorRef?.handleExecuteQuery()}>
								<Play class="w-4 h-4" />
								Run Query
							</Button>
							<Button variant="outline" class="gap-2">
								<Save class="w-4 h-4" />
								Save Script
							</Button>
							
							{#if selectedConnection}
								{@const connection = connections.find(c => c.id === selectedConnection)}
								<div class="ml-auto flex items-center gap-2 text-sm text-gray-600">
									<div class="w-2 h-2 rounded-full bg-green-500"></div>
									Connected to: {connection?.name}
								</div>
							{:else}
								<div class="ml-auto text-sm text-gray-500">
									Select a connection to start
								</div>
							{/if}
						</div>
					</div>

					<!-- Editor and Results -->
					<div class="flex-1 flex flex-col">
						<SqlEditor {selectedConnection} {connections} bind:this={sqlEditorRef} />
					</div>
				</div>
			</ResizablePane>
		</ResizablePaneGroup>
	{/if}
</div>

<!-- Connection Form Modal -->
{#if showConnectionForm}
	<div class="fixed inset-0 bg-black bg-opacity-20 flex items-center justify-center z-50">
		<div class="bg-white rounded-lg shadow-xl p-6 max-w-lg w-full mx-4 max-h-[90vh] overflow-y-auto">
			<ConnectionForm 
				onSubmit={addConnection}
				onCancel={() => showConnectionForm = false}
			/>
		</div>
	</div>
{/if} 