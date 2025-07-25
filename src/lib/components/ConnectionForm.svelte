<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Database, X, CheckCircle, AlertCircle, Info } from '@lucide/svelte';
	import { DatabaseCommands, type ConnectionConfig } from '$lib/commands.svelte';

	interface Props {
		onSubmit: (connection: ConnectionConfig) => void;
		onCancel: () => void;
	}

	let { onSubmit, onCancel }: Props = $props();

	let connectionString = $state('postgresql://username:password@localhost:5432/database');
	let connectionName = $state('');
	let errors = $state<Record<string, string>>({});
	let isTestingConnection = $state(false);
	let testResult = $state<'success' | 'error' | null>(null);

	function validateForm(): boolean {
		errors = {};
		
		if (!connectionName.trim()) {
			errors.name = 'Connection name is required';
		}
		
		if (!connectionString.trim()) {
			errors.connectionString = 'Connection string is required';
		}

		return Object.keys(errors).length === 0;
	}

	async function testConnection() {
		if (!validateForm()) return;

		isTestingConnection = true;
		testResult = null;

		const config: ConnectionConfig = {
			name: connectionName.trim(),
			connection_string: connectionString.trim()
		};

		try {
			const success = await DatabaseCommands.testConnection(config);
			testResult = success ? 'success' : 'error';
		} catch (error) {
			console.error('Connection test failed:', error);
			testResult = 'error';
		} finally {
			isTestingConnection = false;
		}
	}

	function handleSubmit(e: Event) {
		e.preventDefault();
		
		if (validateForm()) {
			const config: ConnectionConfig = {
				name: connectionName.trim(),
				connection_string: connectionString.trim()
			};
			onSubmit(config);
		}
	}
</script>

<form onsubmit={handleSubmit} class="space-y-4">
	<div class="flex items-center justify-between mb-6">
		<div class="flex items-center gap-2">
			<Database class="w-5 h-5 text-blue-600" />
			<h2 class="text-lg font-semibold">Add Connection</h2>
		</div>
		<Button
			type="button"
			variant="ghost"
			size="sm"
			onclick={onCancel}
			class="p-1"
		>
			<X class="w-4 h-4" />
		</Button>
	</div>

	<div class="space-y-4">
		<div>
			<label for="name" class="block text-sm font-medium text-gray-700 mb-1">
				Connection Name *
			</label>
			<Input
				id="name"
				type="text"
				bind:value={connectionName}
				placeholder="e.g., Local Development"
				class={errors.name ? 'border-red-500' : ''}
			/>
			{#if errors.name}
				<p class="text-sm text-red-600 mt-1">{errors.name}</p>
			{/if}
		</div>

		<div>
			<label for="connectionString" class="block text-sm font-medium text-gray-700 mb-1">
				PostgreSQL Connection String *
			</label>
			<Input
				id="connectionString"
				type="text"
				bind:value={connectionString}
				placeholder="postgresql://username:password@localhost:5432/database"
				class={errors.connectionString ? 'border-red-500' : ''}
			/>
			{#if errors.connectionString}
				<p class="text-sm text-red-600 mt-1">{errors.connectionString}</p>
			{/if}
			
			<div class="mt-2 p-3 bg-blue-50 rounded-md">
				<div class="flex items-start gap-2">
					<Info class="w-4 h-4 text-blue-600 mt-0.5 flex-shrink-0" />
					<div class="text-sm text-blue-800">
						<p class="font-medium mb-1">Connection String Format:</p>
						<code class="text-xs bg-blue-100 px-1 py-0.5 rounded">
							postgresql://username:password@hostname:port/database
						</code>
						<p class="mt-1 text-xs">
							Example: <code class="bg-blue-100 px-1 py-0.5 rounded">postgresql://postgres:mypass@localhost:5432/mydb</code>
						</p>
					</div>
				</div>
			</div>
		</div>

		<!-- Test Connection -->
		<div class="border-t pt-4">
			<Button 
				type="button" 
				variant="outline" 
				onclick={testConnection}
				disabled={isTestingConnection || !connectionName.trim() || !connectionString.trim()}
				class="w-full mb-3"
			>
				{#if isTestingConnection}
					Testing Connection...
				{:else}
					Test Connection
				{/if}
			</Button>

			{#if testResult === 'success'}
				<div class="flex items-center gap-2 text-green-600 text-sm">
					<CheckCircle class="w-4 h-4" />
					Connection successful!
				</div>
			{:else if testResult === 'error'}
				<div class="flex items-center gap-2 text-red-600 text-sm">
					<AlertCircle class="w-4 h-4" />
					Connection failed. Please check your connection string.
				</div>
			{/if}
		</div>
	</div>

	<div class="flex gap-2 pt-4">
		<Button type="submit" class="flex-1" disabled={!connectionName.trim() || !connectionString.trim()}>
			Add Connection
		</Button>
		<Button type="button" variant="outline" onclick={onCancel}>
			Cancel
		</Button>
	</div>
</form> 