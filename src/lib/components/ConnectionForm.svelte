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

<form onsubmit={handleSubmit} class="space-y-6">
	<div class="flex items-center justify-between mb-8">
		<div class="flex items-center gap-3">
			<div class="p-2 rounded-lg bg-primary/10 border border-primary/20">
				<Database class="w-5 h-5 text-primary" />
			</div>
			<h2 class="text-xl font-bold text-foreground">Add Connection</h2>
		</div>
		<Button
			type="button"
			variant="ghost"
			size="icon-sm"
			onclick={onCancel}
			class="hover:shadow-md"
		>
			<X class="w-4 h-4" />
		</Button>
	</div>

	<div class="space-y-6">
		<div>
			<label for="name" class="block text-sm font-semibold text-foreground mb-2">
				Connection Name <span class="text-error">*</span>
			</label>
			<Input
				id="name"
				type="text"
				bind:value={connectionName}
				placeholder="e.g., Local Development"
				class={`shadow-sm focus:shadow-md transition-shadow ${errors.name ? 'border-error focus:ring-error/30' : 'focus:ring-primary/30'}`}
			/>
			{#if errors.name}
				<p class="text-sm text-error mt-2 flex items-center gap-2">
					<AlertCircle class="w-4 h-4" />
					{errors.name}
				</p>
			{/if}
		</div>

		<div>
			<label for="connectionString" class="block text-sm font-semibold text-foreground mb-2">
				PostgreSQL Connection String <span class="text-error">*</span>
			</label>
			<Input
				id="connectionString"
				type="text"
				bind:value={connectionString}
				placeholder="postgresql://username:password@localhost:5432/database"
				class={`shadow-sm focus:shadow-md transition-shadow ${errors.connectionString ? 'border-error focus:ring-error/30' : 'focus:ring-primary/30'}`}
			/>
			{#if errors.connectionString}
				<p class="text-sm text-error mt-2 flex items-center gap-2">
					<AlertCircle class="w-4 h-4" />
					{errors.connectionString}
				</p>
			{/if}
			
			<div class="mt-4 p-4 bg-primary/5 border border-primary/20 rounded-lg">
				<div class="flex items-start gap-3">
					<Info class="w-4 h-4 text-primary mt-0.5 flex-shrink-0" />
					<div class="text-sm text-muted-foreground">
						<p class="font-medium text-foreground mb-1">Connection String Format:</p>
						<p class="mb-2">Use the following format for your PostgreSQL connection:</p>
						<code class="block bg-muted/50 p-2 rounded text-xs font-mono border">
							postgresql://username:password@host:port/database
						</code>
						<p class="mt-2 text-xs">
							Replace <span class="font-medium">username</span>, <span class="font-medium">password</span>, 
							<span class="font-medium">host</span>, <span class="font-medium">port</span>, and 
							<span class="font-medium">database</span> with your actual connection details.
						</p>
					</div>
				</div>
			</div>
		</div>
	</div>

	<div class="flex items-center gap-3 pt-6 border-t border-border/50">
		<Button
			type="button"
			variant="outline"
			onclick={testConnection}
			disabled={isTestingConnection}
			class="gap-2 shadow-sm hover:shadow-md"
		>
			{#if isTestingConnection}
				<div class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin"></div>
				Testing...
			{:else}
				Test Connection
			{/if}
		</Button>

		{#if testResult === 'success'}
			<div class="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-success-light/50 border border-success/20">
				<CheckCircle class="w-4 h-4 text-success" />
				<span class="text-sm font-medium text-success-foreground/80">Connection successful!</span>
			</div>
		{:else if testResult === 'error'}
			<div class="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-error-light/50 border border-error/20">
				<AlertCircle class="w-4 h-4 text-error" />
				<span class="text-sm font-medium text-error-foreground/80">Connection failed</span>
			</div>
		{/if}

		<div class="flex-1"></div>

		<Button
			type="button"
			variant="ghost"
			onclick={onCancel}
			class="shadow-sm hover:shadow-md"
		>
			Cancel
		</Button>
		<Button
			type="submit"
			class="gap-2 shadow-md hover:shadow-lg"
		>
			<Database class="w-4 h-4" />
			Add Connection
		</Button>
	</div>
</form> 