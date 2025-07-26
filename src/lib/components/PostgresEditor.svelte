<script lang="ts">
	import { Database, Plus, Play, Save, ChevronLeft, ChevronRight } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import { ResizablePaneGroup, ResizablePane, ResizableHandle } from '$lib/components/ui/resizable';
	import ConnectionSidebar from './ConnectionSidebar.svelte';
	import SqlEditor from './SqlEditor.svelte';
	import ConnectionForm from './ConnectionForm.svelte';
	import { DatabaseCommands, type ConnectionInfo, type ConnectionConfig } from '$lib/commands.svelte';
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
	
	let isSidebarCollapsed = $state(false);
	let lastResizeTime = $state(0);

	// Auto-collapse if resized below 12%
	const COLLAPSE_THRESHOLD = 12;
	// Auto-expand if resized above 10%
	const EXPAND_THRESHOLD = 10;

	$effect(() => {
		if (selectedConnection) {
			const connection = connections.find(c => c.id === selectedConnection);
			if (connection) {
				currentConnection = {
					name: connection.name,
					connected: connection.connected
				};
				isConnecting = establishingConnections.has(connection.id);
			}
		} else {
			currentConnection = null;
			isConnecting = false;
		}
	});

	function handlePaneResize(sizes: number[]) {
		const now = Date.now();
		lastResizeTime = now;
		
		// Some 'debounce' logic because the cursor was getting stuck for some reason
		setTimeout(() => {
			if (now === lastResizeTime && sizes.length >= 2) {
				const sidebarSize = sizes[0];
				
				if (!isSidebarCollapsed && sidebarSize < COLLAPSE_THRESHOLD) {
					isSidebarCollapsed = true;
				}
				else if (isSidebarCollapsed && sidebarSize > EXPAND_THRESHOLD) {
					isSidebarCollapsed = false;
				}
			}
		}, 150);
	}

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

<div class="flex h-screen bg-gradient-to-br from-background via-background to-muted/20">
	<ResizablePaneGroup direction="horizontal" class="flex-1" onLayoutChange={handlePaneResize}>
		<!-- Sidebar Pane - always present but with conditional behavior -->
		<ResizablePane 
			defaultSize={isSidebarCollapsed ? 4 : 25} 
			minSize={isSidebarCollapsed ? 4 : 8} 
			maxSize={isSidebarCollapsed ? 40 : 40}
			class="transition-all duration-300 ease-out"
		>
			<div class="h-full bg-sidebar/80 glass-subtle border-r border-sidebar-border flex flex-col relative">
				{#if isSidebarCollapsed}
					<!-- Collapsed Sidebar Content -->
					<button
						class="absolute top-4 right-2 z-10 p-2 rounded-lg hover:bg-sidebar-accent/80 transition-all duration-200 hover:shadow-md"
						onclick={toggleSidebar}
						title="Expand sidebar"
					>
						<ChevronRight class="w-4 h-4 text-sidebar-foreground/70" />
					</button>

					<div class="p-3 border-b border-sidebar-border/50">
						<div class="flex flex-col items-center gap-3">
							<div class="p-2 rounded-lg bg-primary/10 border border-primary/20">
								<Database class="w-6 h-6 text-primary" />
							</div>
							<Button
								size="icon-sm"
								variant="outline"
								class="shadow-md hover:shadow-lg"
								onclick={() => showConnectionForm = true}
								title="Add Connection"
							>
								<Plus class="w-4 h-4" />
							</Button>
						</div>
					</div>
					
					<div class="flex-1 p-3 space-y-3">
						{#each connections as connection}
							<button
								class="w-full h-10 rounded-lg flex items-center justify-center transition-all duration-200 hover:shadow-md {selectedConnection === connection.id ? 'bg-primary/20 border border-primary/30 shadow-md' : 'hover:bg-sidebar-accent/60'}"
								onclick={() => selectConnection(connection.id)}
								title={connection.name}
							>
								<div class="w-3 h-3 rounded-full shadow-sm {connection.connected ? 'bg-success shadow-success/30' : establishingConnections.has(connection.id) ? 'bg-primary animate-pulse shadow-primary/30' : 'bg-muted-foreground/40'}"></div>
							</button>
						{/each}
					</div>
				{:else}
					<!-- Expanded Sidebar Content -->
					<button
						class="absolute top-6 right-4 z-10 p-2 rounded-lg hover:bg-sidebar-accent/80 transition-all duration-200 hover:shadow-md"
						onclick={toggleSidebar}
						title="Collapse sidebar"
					>
						<ChevronLeft class="w-4 h-4 text-sidebar-foreground/70" />
					</button>

					<div class="p-6 border-b border-sidebar-border/50">
						<div class="flex items-center gap-3 mb-6">
							<div class="p-2 rounded-lg bg-primary/10 border border-primary/20">
								<Database class="w-6 h-6 text-primary" />
							</div>
							<h1 class="text-xl font-bold text-sidebar-foreground">PgPad</h1>
						</div>
						
						<Button
							class="w-full justify-start gap-2 shadow-md hover:shadow-lg"
							variant="outline"
							onclick={() => showConnectionForm = true}
						>
							<Plus class="w-4 h-4" />
							Add Connection
						</Button>
					</div>

					<ConnectionSidebar
						{connections}
						{selectedConnection}
						{establishingConnections}
						onSelect={selectConnection}
						onConnect={connectToDatabase}
					/>
				{/if}
			</div>
		</ResizablePane>

		<ResizableHandle />

		<!-- Main Editor Pane - always in same position -->
		<ResizablePane defaultSize={isSidebarCollapsed ? 96 : 75}>
			<div class="flex flex-col h-full bg-background/50">
				<!-- Toolbar -->
				<div class="glass-card border-b border-border/50 p-6 shadow-md">
					<div class="flex items-center justify-between">
						<div class="flex items-center gap-4">
							<div class="flex items-center gap-3">
								<Button 
									class="gap-2 shadow-md hover:shadow-lg" 
									disabled={!selectedConnection} 
									onclick={() => sqlEditorRef?.handleExecuteQuery()}
									title="Run Query (Ctrl+R for full script, Ctrl+Enter for selection)"
								>
									<Play class="w-4 h-4" />
									Run Query
								</Button>
								<Button variant="outline" class="gap-2 shadow-sm hover:shadow-md">
									<Save class="w-4 h-4" />
									Save Script
								</Button>
							</div>
							
							{#if selectedConnection}
								{@const connection = connections.find(c => c.id === selectedConnection)}
								{#if connection}
									{#if connection.connected}
										<div class="flex items-center gap-2 px-4 py-2 rounded-lg bg-success-light/20 border border-success/30">
											<div class="w-2 h-2 rounded-full bg-success shadow-sm"></div>
											<span class="text-sm font-semibold text-foreground">Connected to: {connection.name}</span>
										</div>
									{:else if establishingConnections.has(connection.id)}
										<div class="flex items-center gap-2 px-4 py-2 rounded-lg bg-primary/10 border border-primary/30">
											<div class="w-2 h-2 rounded-full bg-primary animate-pulse shadow-sm"></div>
											<span class="text-sm font-semibold text-foreground">Connecting to: {connection.name}</span>
										</div>
									{:else}
										<div class="flex items-center gap-2 px-4 py-2 rounded-lg bg-muted/20 border border-border">
											<div class="w-2 h-2 rounded-full bg-muted-foreground/60 shadow-sm"></div>
											<span class="text-sm font-semibold text-foreground">Selected: {connection.name} (double-click to connect)</span>
										</div>
									{/if}
								{/if}
							{:else}
								<div class="flex items-center gap-2 px-4 py-2 rounded-lg bg-muted/30 border border-border">
									<span class="text-sm font-semibold text-muted-foreground">Select a connection to start</span>
								</div>
							{/if}
						</div>
						{#if isSidebarCollapsed}
							<div class="flex items-center gap-3">
								<Button
									variant="ghost"
									size="sm"
									onclick={toggleSidebar}
									class="hover:shadow-md"
								>
									<ChevronRight class="w-4 h-4" />
									Show Sidebar
								</Button>
							</div>
						{/if}
					</div>
				</div>

				<!-- Editor and Results - same component instance always -->
				<div class="flex-1 flex flex-col bg-background/30">
					<SqlEditor {selectedConnection} {connections} bind:this={sqlEditorRef} />
				</div>
			</div>
		</ResizablePane>
	</ResizablePaneGroup>
</div>

<!-- Connection Form Modal -->
{#if showConnectionForm}
	<div class="fixed inset-0 bg-background/80 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in">
		<div class="glass-card rounded-xl shadow-xl p-8 max-w-lg w-full mx-4 max-h-[90vh] overflow-y-auto hover-lift">
			<ConnectionForm 
				onSubmit={addConnection}
				onCancel={() => showConnectionForm = false}
			/>
		</div>
	</div>
{/if}