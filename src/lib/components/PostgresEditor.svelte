<script lang="ts">
	import { Database, Plus, Play, Save } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import ConnectionSidebar from './ConnectionSidebar.svelte';
	import SqlEditor from './SqlEditor.svelte';
	import ConnectionForm from './ConnectionForm.svelte';

	let showConnectionForm = $state(false);
	let selectedConnection = $state<string | null>(null);
	let connections = $state([
		{
			id: '1',
			name: 'Local Development',
			host: 'localhost',
			port: 5432,
			database: 'myapp_dev',
			username: 'postgres',
			connected: true
		},
		{
			id: '2', 
			name: 'Production',
			host: 'prod-db.example.com',
			port: 5432,
			database: 'myapp_prod',
			username: 'app_user',
			connected: false
		}
	]);

	function addConnection(connection: any) {
		connections.push({
			...connection,
			id: Date.now().toString(),
			connected: false
		});
		showConnectionForm = false;
	}

	function selectConnection(connectionId: string) {
		selectedConnection = connectionId;
	}
</script>

<div class="flex h-screen bg-gray-50">
	<!-- Sidebar -->
	<div class="w-80 bg-white border-r border-gray-200 flex flex-col">
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
			onSelect={selectConnection}
		/>
	</div>

	<!-- Main Editor Area -->
	<div class="flex-1 flex flex-col">
		<!-- Toolbar -->
		<div class="bg-white border-b border-gray-200 p-4">
			<div class="flex items-center gap-2">
				<Button class="gap-2" disabled={!selectedConnection}>
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
			<SqlEditor {selectedConnection} />
		</div>
	</div>
</div>

<!-- Connection Form Modal -->
{#if showConnectionForm}
	<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
		<div class="bg-white rounded-lg p-6 max-w-md w-full mx-4">
			<ConnectionForm 
				onSubmit={addConnection}
				onCancel={() => showConnectionForm = false}
			/>
		</div>
	</div>
{/if} 