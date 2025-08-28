<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Cable, X, CheckCircle, AlertCircle, Info, Database, Server, FolderOpen } from '@lucide/svelte';
	import { Commands, type ConnectionConfig, type ConnectionInfo } from '$lib/commands.svelte';
	import { Tabs } from 'bits-ui';

	interface Props {
		onSubmit: (connection: ConnectionConfig) => void;
		onCancel: () => void;
		editingConnection?: ConnectionInfo | null;
	}

	let { onSubmit, onCancel, editingConnection = null }: Props = $props();

	let connectionString = $state(
		editingConnection?.connection_string || 'postgresql://username:password@localhost:5432/database'
	);
	let connectionName = $state(editingConnection?.name || '');
	// For now, assume all existing connections are Postgres since SQLite support is new
	// TODO: Update this when ConnectionInfo includes database type
	let databaseType = $state<'postgres' | 'sqlite'>(
		editingConnection ? 'postgres' : 'postgres'
	);
	let sqliteFilePath = $state<string>('');
	let errors = $state<Record<string, string>>({});
	let isTestingConnection = $state(false);
	let testResult = $state<'success' | 'error' | null>(null);

	const isEditing = $derived(editingConnection !== null);
	const modalTitle = $derived(isEditing ? 'Edit Connection' : 'Add Connection');
	const submitButtonText = $derived(isEditing ? 'Update Connection' : 'Add Connection');

	function validateForm(): boolean {
		errors = {};

		if (!connectionName.trim()) {
			errors.name = 'Connection name is required';
		}

		// Only validate connection string for PostgreSQL
		if (databaseType === 'postgres' && !connectionString.trim()) {
			errors.connectionString = 'Connection string is required';
		}

		// For SQLite, validate file path
		if (databaseType === 'sqlite' && !sqliteFilePath.trim()) {
			errors.sqliteFilePath = 'SQLite database file is required';
		}

		return Object.keys(errors).length === 0;
	}

	async function openFileDialog() {
		try {
			const selectedPath = await Commands.openFileDialog();
			if (selectedPath) {
				sqliteFilePath = selectedPath;
				// Clear any existing errors when a file is selected
				if (errors.sqliteFilePath) {
					errors = { ...errors };
					delete errors.sqliteFilePath;
				}
			}
		} catch (error) {
			console.error('Failed to open file dialog:', error);
		}
	}

	async function testConnection() {
		if (!validateForm()) return;

		// For now, only allow testing PostgreSQL connections
		if (databaseType !== 'postgres') {
			return;
		}

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
				connection_string: databaseType === 'postgres' 
					? connectionString.trim() 
					: sqliteFilePath.trim()
			};
			onSubmit(config);
		}
	}
</script>

<form onsubmit={handleSubmit} class="space-y-5">
	<div class="mb-6 flex items-center justify-between">
		<div class="flex items-center gap-3">
			<div class="bg-primary/10 border-primary/20 rounded-lg border p-2">
				<Cable class="text-primary h-5 w-5" />
			</div>
			<h2 class="text-foreground text-xl font-bold">{modalTitle}</h2>
		</div>
		<Button type="button" variant="ghost" size="icon-sm" onclick={onCancel} class="hover:shadow-md">
			<X class="h-4 w-4" />
		</Button>
	</div>

	<div class="space-y-4">
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
			<div class="text-foreground mb-2 block text-sm font-semibold">
				Database Type <span class="text-error">*</span>
			</div>
			
			<Tabs.Root bind:value={databaseType} class="w-full">
				<Tabs.List class="bg-muted/20 grid w-full grid-cols-2 gap-1 rounded-lg p-1">
					<Tabs.Trigger 
						value="postgres" 
						class="data-[state=active]:bg-foreground data-[state=active]:text-background data-[state=active]:shadow-lg data-[state=inactive]:hover:bg-muted/30 data-[state=inactive]:text-muted-foreground flex items-center justify-center gap-2 rounded-md px-4 py-2.5 text-sm font-semibold transition-all duration-200"
					>
						<Server class="h-4 w-4" />
						PostgreSQL
					</Tabs.Trigger>
					<Tabs.Trigger 
						value="sqlite" 
						class="data-[state=active]:bg-foreground data-[state=active]:text-background data-[state=active]:shadow-lg data-[state=inactive]:hover:bg-muted/30 data-[state=inactive]:text-muted-foreground flex items-center justify-center gap-2 rounded-md px-4 py-2.5 text-sm font-semibold transition-all duration-200"
					>
						<Database class="h-4 w-4" />
						SQLite
					</Tabs.Trigger>
				</Tabs.List>
				
				<Tabs.Content value="postgres" class="mt-3">
					<div class="rounded-xl border bg-card p-5 shadow-sm">
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

						<div class="bg-primary/5 border-primary/20 mt-3 rounded-lg border p-3">
							<div class="flex items-start gap-3">
								<Info class="text-primary mt-0.5 h-4 w-4 flex-shrink-0" />
								<div class="text-muted-foreground text-sm min-w-0 flex-1">
									<p class="text-foreground mb-2 font-medium">Connection String Format:</p>
									<p class="mb-2">Use the following format for your PostgreSQL connection:</p>
									<div class="bg-muted/50 rounded border p-2 font-mono text-xs overflow-x-auto">
										<code class="whitespace-nowrap">
											postgresql://username:password@host:port/database
										</code>
									</div>
									<p class="mt-2 text-xs leading-relaxed">
										Replace <span class="font-medium text-foreground">username</span>,
										<span class="font-medium text-foreground">password</span>,
										<span class="font-medium text-foreground">host</span>, 
										<span class="font-medium text-foreground">port</span>, and
										<span class="font-medium text-foreground">database</span> with your actual connection details.
									</p>
								</div>
							</div>
						</div>
					</div>
				</Tabs.Content>
				
				<Tabs.Content value="sqlite" class="mt-3">
					<div class="rounded-xl border bg-card p-5 shadow-sm">
						<label for="sqliteFilePath" class="text-foreground mb-2 block text-sm font-semibold">
							SQLite Database File <span class="text-error">*</span>
						</label>
						
						<div class="flex gap-3">
							<Input
								id="sqliteFilePath"
								type="text"
								bind:value={sqliteFilePath}
								placeholder="Select a SQLite database file..."
								readonly
								class={`flex-1 cursor-pointer shadow-sm transition-shadow focus:shadow-md ${errors.sqliteFilePath ? 'border-error' : ''}`}
								onclick={openFileDialog}
							/>
							<Button
								type="button"
								variant="outline"
								onclick={openFileDialog}
								class="gap-2 shadow-sm hover:shadow-md"
							>
								<FolderOpen class="h-4 w-4" />
								Browse
							</Button>
						</div>
						
						{#if errors.sqliteFilePath}
							<p class="text-error mt-2 flex items-center gap-2 text-sm">
								<AlertCircle class="h-4 w-4" />
								{errors.sqliteFilePath}
							</p>
						{/if}

						<div class="bg-primary/5 border-primary/20 mt-3 rounded-lg border p-3">
							<div class="flex items-start gap-3">
								<Info class="text-primary mt-0.5 h-4 w-4 flex-shrink-0" />
								<div class="text-muted-foreground text-sm min-w-0 flex-1">
									<p class="text-foreground mb-2 font-medium">SQLite Database File:</p>
									<p class="mb-2 leading-relaxed">Select an existing SQLite database file or choose a location to create a new one.</p>
									<p class="text-xs leading-relaxed">
										SQLite databases are single files with <span class="font-medium text-foreground">.db</span>, 
										<span class="font-medium text-foreground">.sqlite</span>, or <span class="font-medium text-foreground">.sqlite3</span> extensions.
									</p>
								</div>
							</div>
						</div>
					</div>
				</Tabs.Content>
			</Tabs.Root>
		</div>
	</div>

	<div class="border-border/50 flex items-center gap-3 border-t pt-5">
		<Button
			type="button"
			variant="outline"
			onclick={testConnection}
			disabled={isTestingConnection || databaseType !== 'postgres'}
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
		<Button 
			type="submit" 
			class="gap-2 shadow-md hover:shadow-lg"
		>
			<Cable class="h-4 w-4" />
			{submitButtonText}
		</Button>
	</div>
</form>
