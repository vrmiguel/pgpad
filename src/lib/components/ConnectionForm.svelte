<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Cable, X, CheckCircle, AlertCircle, Info, FolderOpen } from '@lucide/svelte';

	import IconCibPostgresql from '~icons/cib/postgresql';
	import IconSimpleIconsSqlite from '~icons/simple-icons/sqlite';

	import { Commands, type DatabaseInfo, type ConnectionInfo } from '$lib/commands.svelte';
	import { Tabs } from 'bits-ui';

	interface Props {
		onSubmit: (name: string, databaseInfo: DatabaseInfo) => void;
		onCancel: () => void;
		editingConnection?: ConnectionInfo | null;
	}

	let { onSubmit, onCancel, editingConnection = null }: Props = $props();

	// Initialize form fields based on editing connection
	let connectionName = $state(editingConnection?.name || '');

	let databaseType = $state<'postgres' | 'sqlite'>('postgres');
	let connectionString = $state('');
	let sqliteFilePath = $state('');

	if (editingConnection) {
		if ('Postgres' in editingConnection.database_type) {
			databaseType = 'postgres';
			connectionString = editingConnection.database_type.Postgres.connection_string;
		} else if ('SQLite' in editingConnection.database_type) {
			databaseType = 'sqlite';
			sqliteFilePath = editingConnection.database_type.SQLite.db_path;
		}
	}
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

		isTestingConnection = true;
		testResult = null;

		const databaseInfo: DatabaseInfo =
			databaseType === 'postgres'
				? { Postgres: { connection_string: connectionString.trim() } }
				: { SQLite: { db_path: sqliteFilePath.trim() } };

		try {
			const success = await Commands.testConnection(databaseInfo);
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
			const databaseInfo: DatabaseInfo =
				databaseType === 'postgres'
					? { Postgres: { connection_string: connectionString.trim() } }
					: { SQLite: { db_path: sqliteFilePath.trim() } };

			onSubmit(connectionName.trim(), databaseInfo);
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
				autofocus
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
						class="data-[state=inactive]:hover:bg-muted/30 data-[state=inactive]:text-muted-foreground flex items-center justify-center gap-2 rounded-md px-4 py-2.5 text-sm font-semibold transition-all duration-200 data-[state=active]:bg-[var(--border)] data-[state=active]:shadow-lg"
					>
						<IconCibPostgresql class="h-4 w-4" />
						PostgreSQL
					</Tabs.Trigger>
					<Tabs.Trigger
						value="sqlite"
						class="data-[state=inactive]:hover:bg-muted/30 data-[state=inactive]:text-muted-foreground flex items-center justify-center gap-2 rounded-md px-4 py-2.5 text-sm font-semibold transition-all duration-200 data-[state=active]:bg-[var(--border)] data-[state=active]:shadow-lg"
					>
						<IconSimpleIconsSqlite class="h-4 w-4" />
						SQLite
					</Tabs.Trigger>
				</Tabs.List>

				<Tabs.Content value="postgres" class="mt-3">
					<div class="bg-card rounded-xl border p-5 shadow-sm">
						<label for="connectionString" class="text-foreground mb-2 block text-sm font-semibold">
							Connection String <span class="text-error">*</span>
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
								<div class="text-muted-foreground min-w-0 flex-1 text-sm">
									<p class="mb-2">Use the following format for your Postgres connection:</p>
									<div class="bg-muted/50 overflow-x-auto rounded border p-2 font-mono text-xs">
										<code class="whitespace-nowrap">
											postgresql://username:password@host:port/database
										</code>
									</div>
									<p class="mt-2 text-xs leading-relaxed">
										Replace <span class="text-foreground font-medium">username</span>,
										<span class="text-foreground font-medium">password</span>,
										<span class="text-foreground font-medium">host</span>,
										<span class="text-foreground font-medium">port</span>, and
										<span class="text-foreground font-medium">database</span> with your actual connection
										details.
									</p>
								</div>
							</div>
						</div>
					</div>
				</Tabs.Content>

				<Tabs.Content value="sqlite" class="mt-3">
					<div class="bg-card rounded-xl border p-5 shadow-sm">
						<label for="sqliteFilePath" class="text-foreground mb-2 block text-sm font-semibold">
							Database File <span class="text-error">*</span>
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
								<div class="text-muted-foreground min-w-0 flex-1 text-sm">
									<p class="mb-2 leading-relaxed">
										Select an existing SQLite database file or choose a location to create a new
										one.
									</p>
									<p class="text-xs leading-relaxed">
										SQLite databases are single files with <span class="text-foreground font-medium"
											>.db</span
										>,
										<span class="text-foreground font-medium">.sqlite</span>, or
										<span class="text-foreground font-medium">.sqlite3</span> extensions.
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

		<Button type="button" variant="outline" onclick={onCancel} class="shadow-sm hover:shadow-md">
			Cancel
		</Button>
		<Button type="submit" class="gap-2 shadow-md hover:shadow-lg">
			<Cable class="h-4 w-4" />
			{submitButtonText}
		</Button>
	</div>
</form>
