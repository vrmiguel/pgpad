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

	import { Button } from '$lib/components/ui/button';
	import DatabaseSchemaItems from './DatabaseSchemaItems.svelte';
	import Logo from './Logo.svelte';
	import type {
		ConnectionInfo,
		Script,
		DatabaseSchema,
		QueryHistoryEntry
	} from '$lib/commands.svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { Tabs } from 'bits-ui';
	import type { SidebarTabState } from '$lib/stores/tabs.svelte';
	import Scripts from './Scripts.svelte';
	import QueryHistory from './QueryHistory.svelte';
	import Connections from './Connections.svelte';

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
		sidebarTabState?: SidebarTabState;

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
		sidebarTabState = $bindable(),
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

	function showConnectionForm() {
		onShowConnectionForm?.();
	}

	function handleTableClick(tableName: string, schema: string) {
		onTableClick?.(tableName, schema);
	}

	function switchTab(state: SidebarTabState) {
		isSidebarCollapsed = false;
		sidebarTabState = state;
	}
</script>

<div class="bg-sidebar/80 glass-subtle border-sidebar-border flex h-full flex-col border-r">
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
				onclick={() => switchTab('connections')}
				title="Connections"
			>
				<Cable
					class="text-sidebar-foreground/70 group-hover:text-primary/90 h-5 w-5 transition-colors duration-200"
				/>
			</button>

			<!-- items icon -->
			<button
				class="group flex h-12 w-12 items-center justify-center rounded-lg transition-all duration-200 ease-out hover:bg-white/3 dark:hover:bg-white/5"
				onclick={() => switchTab('items')}
				title="Database Items"
			>
				<TableProperties
					class="text-sidebar-foreground/70 group-hover:text-primary/90 h-5 w-5 transition-colors duration-200"
				/>
			</button>

			<!-- Scripts icon -->
			<button
				class="group flex h-12 w-12 items-center justify-center rounded-lg transition-all duration-200 ease-out hover:bg-white/3 dark:hover:bg-white/5"
				onclick={() => switchTab('scripts')}
				title="Scripts"
			>
				<FileJson
					class="text-sidebar-foreground/70 group-hover:text-primary/90 h-5 w-5 transition-colors duration-200"
				/>
			</button>

			<!-- History icon -->
			<button
				class="group flex h-12 w-12 items-center justify-center rounded-lg transition-all duration-200 ease-out hover:bg-white/3 dark:hover:bg-white/5"
				onclick={() => switchTab('history')}
				title="Query History"
			>
				<History
					class="text-sidebar-foreground/70 group-hover:text-primary/90 h-5 w-5 transition-colors duration-200"
				/>
			</button>
		</div>
	{:else}
		<!-- expanded sidebar content -->
		<div class="border-sidebar-border/50 flex justify-between border-b p-6">
			<div class="flex items-center gap-4">
				<Logo
					class="text-primary/90 hover:text-primary drop-shadow-lg transition-all duration-300 hover:scale-105"
					size="md"
				/>
				<h1 class="text-sidebar-foreground text-xl font-bold">PgPad</h1>
			</div>
			<button
				class="hover:bg-sidebar-accent/80 rounded-lg p-2 transition-all duration-200 hover:shadow-md"
				onclick={toggleSidebar}
				title="Collapse sidebar"
			>
				<ChevronLeft class="text-sidebar-foreground/70 h-4 w-4" />
			</button>
		</div>

		<div class="min-h-0 flex-1">
			<Tabs.Root value={sidebarTabState} class="flex h-full flex-col">
				<!-- Triggers  -->
				<div class="border-b p-2">
					<Tabs.List
						class="bg-dark-10 shadow-mini-inset bg-background flex h-8 w-full flex-row justify-evenly gap-1 rounded-sm border-none p-0.5 text-sm leading-[0.01em] font-semibold dark:border dark:border-neutral-600/30"
					>
						<Tabs.Trigger
							value="connections"
							title="Connections"
							class="dark:data-[state=active]:bg-muted flex w-20 items-center justify-center rounded-[7px] bg-transparent data-[state=active]:bg-white data-[state=active]:shadow"
						>
							<Cable class="text-sidebar-foreground/70 w-4" />
						</Tabs.Trigger>
						<Tabs.Trigger
							value="items"
							title="Items"
							class="dark:data-[state=active]:bg-muted flex w-20 items-center justify-center rounded-[7px] bg-transparent data-[state=active]:bg-white data-[state=active]:shadow"
						>
							<TableProperties class="text-sidebar-foreground/70 w-4" />
						</Tabs.Trigger>
						<Tabs.Trigger
							value="scripts"
							title="Scripts"
							class="dark:data-[state=active]:bg-muted flex w-20 items-center justify-center rounded-[7px] bg-transparent data-[state=active]:bg-white data-[state=active]:shadow"
						>
							<FileJson class="text-sidebar-foreground/70 w-4" />
						</Tabs.Trigger>
						<Tabs.Trigger
							value="history"
							title="History"
							class="dark:data-[state=active]:bg-muted flex w-20 items-center justify-center rounded-[7px] bg-transparent data-[state=active]:bg-white data-[state=active]:shadow"
						>
							<History class="text-sidebar-foreground/70 w-4" />
						</Tabs.Trigger>
					</Tabs.List>
				</div>

				<!-- Contents -->
				<div class="min-h-0 flex-1 p-2">
					<Tabs.Content value="connections" class="h-full">
						<Connections
							{connections}
							{establishingConnections}
							{selectedConnection}
							{onDisconnectConnection}
							{onDeleteConnection}
							{onEditConnection}
							{onConnectToDatabase}
							{showConnectionForm}
							{onSelectConnection}
						/>
					</Tabs.Content>
					<Tabs.Content value="items" class="h-full">
						<DatabaseSchemaItems
							{databaseSchema}
							{loadingSchema}
							{selectedConnection}
							onTableClick={handleTableClick}
						/>
					</Tabs.Content>
					<Tabs.Content value="scripts" class="h-full">
						<Scripts
							{scripts}
							{activeScriptId}
							{unsavedChanges}
							{onCreateNewScript}
							{onDeleteScript}
							{onSelectScript}
						/>
					</Tabs.Content>
					<Tabs.Content value="history" class="h-full">
						<QueryHistory {queryHistory} {onLoadFromHistory} />
					</Tabs.Content>
				</div>
			</Tabs.Root>
		</div>
	{/if}
</div>
