<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Database, X, CheckCircle, AlertCircle, Info } from '@lucide/svelte';
	import { Commands, type ConnectionConfig } from '$lib/commands.svelte';

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
			const success = await Commands.testConnection(config);
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
	<div class="mb-8 flex items-center justify-between">
		<div class="flex items-center gap-3">
			<div class="bg-primary/10 border-primary/20 rounded-lg border p-2">
				<Database class="text-primary h-5 w-5" />
			</div>
			<h2 class="text-foreground text-xl font-bold">Add Connection</h2>
		</div>
		<Button type="button" variant="ghost" size="icon-sm" onclick={onCancel} class="hover:shadow-md">
			<X class="h-4 w-4" />
		</Button>
	</div>

	<div class="space-y-6">
		<div>
			<label for="name" class="text-foreground mb-2 block text-sm font-semibold">
				Connection Name <span class="text-error">*</span>
			</label>
			<Input
				id="name"
				type="text"
				bind:value={connectionName}
				placeholder="e.g., Local Development"
				class={`shadow-sm transition-shadow focus:shadow-md ${errors.name ? 'border-error focus:ring-error/30' : 'focus:ring-primary/30'}`}
			/>
			{#if errors.name}
				<p class="text-error mt-2 flex items-center gap-2 text-sm">
					<AlertCircle class="h-4 w-4" />
					{errors.name}
				</p>
			{/if}
		</div>

		<div>
			<label for="connectionString" class="text-foreground mb-2 block text-sm font-semibold">
				PostgreSQL Connection String <span class="text-error">*</span>
			</label>
			<Input
				id="connectionString"
				type="text"
				bind:value={connectionString}
				placeholder="postgresql://username:password@localhost:5432/database"
				class={`shadow-sm transition-shadow focus:shadow-md ${errors.connectionString ? 'border-error focus:ring-error/30' : 'focus:ring-primary/30'}`}
			/>
			{#if errors.connectionString}
				<p class="text-error mt-2 flex items-center gap-2 text-sm">
					<AlertCircle class="h-4 w-4" />
					{errors.connectionString}
				</p>
			{/if}

			<div class="bg-primary/5 border-primary/20 mt-4 rounded-lg border p-4">
				<div class="flex items-start gap-3">
					<Info class="text-primary mt-0.5 h-4 w-4 flex-shrink-0" />
					<div class="text-muted-foreground text-sm">
						<p class="text-foreground mb-1 font-medium">Connection String Format:</p>
						<p class="mb-2">Use the following format for your PostgreSQL connection:</p>
						<code class="bg-muted/50 block rounded border p-2 font-mono text-xs">
							postgresql://username:password@host:port/database
						</code>
						<p class="mt-2 text-xs">
							Replace <span class="font-medium">username</span>,
							<span class="font-medium">password</span>,
							<span class="font-medium">host</span>, <span class="font-medium">port</span>, and
							<span class="font-medium">database</span> with your actual connection details.
						</p>
					</div>
				</div>
			</div>
		</div>
	</div>

	<div class="border-border/50 flex items-center gap-3 border-t pt-6">
		<Button
			type="button"
			variant="outline"
			onclick={testConnection}
			disabled={isTestingConnection}
			class="gap-2 shadow-sm hover:shadow-md"
		>
			{#if isTestingConnection}
				<div
					class="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"
				></div>
				Testing...
			{:else}
				Test Connection
			{/if}
		</Button>

		{#if testResult === 'success'}
			<div
				class="bg-success/35 border-success/40 flex items-center gap-2 rounded-lg border px-3 py-1.5"
			>
				<CheckCircle class="text-success h-4 w-4" />
				<span class="text-success-foreground/80 text-sm font-medium">Connection successful!</span>
			</div>
		{:else if testResult === 'error'}
			<div
				class="bg-error/35 border-error/40 text-error-foreground flex items-center gap-2 rounded-lg border px-3 py-1.5"
			>
				<AlertCircle class="text-error h-4 w-4" />
				<span class="text-error-foreground/80 text-sm font-medium">Connection failed</span>
			</div>
		{/if}

		<div class="flex-1"></div>

		<Button type="button" variant="ghost" onclick={onCancel} class="shadow-sm hover:shadow-md">
			Cancel
		</Button>
		<Button type="submit" class="gap-2 shadow-md hover:shadow-lg">
			<Database class="h-4 w-4" />
			Add Connection
		</Button>
	</div>
</form>
