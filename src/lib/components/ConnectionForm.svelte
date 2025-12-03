<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import FormRow from '$lib/components/ui/FormRow.svelte';
	import {
		Cable,
		X,
		CheckCircle,
		AlertCircle,
		FolderOpen,
		FilePlus,
		FileCheck
	} from '@lucide/svelte';

	import IconCibPostgresql from '~icons/cib/postgresql';
	import IconSimpleIconsSqlite from '~icons/simple-icons/sqlite';
	import IconSimpleIconsDuckdb from '~icons/simple-icons/duckdb';
	import IconSimpleIconsMssql from '~icons/simple-icons/microsoftsqlserver';

	import {
		Commands,
		type DatabaseInfo,
		type ConnectionInfo,
		type OracleSettings
	} from '$lib/commands.svelte';
	import { Tabs } from 'bits-ui';

	interface Props {
		onSubmit: (name: string, databaseInfo: DatabaseInfo) => void;
		onCancel: () => void;
		editingConnection?: ConnectionInfo | null;
	}

	let { onSubmit, onCancel, editingConnection = null }: Props = $props();

	let connectionName = $state(editingConnection?.name || '');

	let databaseType = $state<'postgres' | 'sqlite' | 'duckdb' | 'oracle' | 'mssql'>('postgres');
	let connectionString = $state('');
	let caCertPath = $state<string>('');
	let sqliteFilePath = $state('');
	let sqliteSqlcipher = $state(false);
	let sqlitePassphrase = $state('');
	let duckdbFilePath = $state('');
	let oracleConnectionString = $state('');
	let oracleWalletPath = $state<string>('');
	let oracleTnsAlias = $state<string>('');

	// Oracle advanced settings (per-connection)
	let oracleSettings: OracleSettings | null = $state(null);
	let loadingOracleSettings = $state(false);
	let savingOracleSettings = $state(false);

	async function loadOracleSettingsForEditing() {
		if (databaseType !== 'oracle' || !editingConnection) return;
		loadingOracleSettings = true;
		try {
			oracleSettings = await Commands.getOracleSettings(editingConnection.id);
		} catch {
			oracleSettings = null;
		} finally {
			loadingOracleSettings = false;
		}
	}

	function updateOracleSetting<K extends keyof OracleSettings>(key: K, value: OracleSettings[K]) {
		if (!oracleSettings) oracleSettings = {} as OracleSettings;
		oracleSettings[key] = value;
	}

	async function saveOracleSettings() {
		if (databaseType !== 'oracle' || !editingConnection || !oracleSettings) return;
		savingOracleSettings = true;
		try {
			await Commands.setOracleSettings(oracleSettings, editingConnection.id);
		} finally {
			savingOracleSettings = false;
		}
	}

	$effect(() => {
		const dt = databaseType;
		const ec = editingConnection;
		if (dt === 'oracle' && ec) {
			loadOracleSettingsForEditing();
		}
	});

	if (editingConnection) {
		if ('Postgres' in editingConnection.database_type) {
			databaseType = 'postgres';
			connectionString = editingConnection.database_type.Postgres.connection_string;
			caCertPath = editingConnection.database_type.Postgres.ca_cert_path || '';
		} else if ('SQLite' in editingConnection.database_type) {
			databaseType = 'sqlite';
			sqliteFilePath = editingConnection.database_type.SQLite.db_path;
		} else if ('DuckDB' in editingConnection.database_type) {
			databaseType = 'duckdb';
			duckdbFilePath = editingConnection.database_type.DuckDB.db_path;
		} else if ('Mssql' in editingConnection.database_type) {
			databaseType = 'mssql';
			connectionString = editingConnection.database_type.Mssql.connection_string;
			caCertPath = editingConnection.database_type.Mssql.ca_cert_path || '';
		} else if ('Oracle' in editingConnection.database_type) {
			databaseType = 'oracle';
			oracleConnectionString = editingConnection.database_type.Oracle.connection_string;
			oracleWalletPath = editingConnection.database_type.Oracle.wallet_path || '';
			oracleTnsAlias = editingConnection.database_type.Oracle.tns_alias || '';
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

		if (databaseType === 'postgres' && !connectionString.trim()) {
			errors.connectionString = 'Connection string is required';
		}

		if (databaseType === 'sqlite' && !sqliteFilePath.trim()) {
			errors.sqliteFilePath = 'SQLite database file is required';
		}
		if (databaseType === 'sqlite' && sqliteSqlcipher && !sqlitePassphrase.trim()) {
			errors.sqlitePassphrase = 'Passphrase is required for SQLCipher';
		}

		if (databaseType === 'duckdb' && !duckdbFilePath.trim()) {
			errors.duckdbFilePath = 'DuckDB database file is required';
		}
		if (databaseType === 'mssql' && !connectionString.trim()) {
			errors.connectionString = 'Connection string is required';
		}
		if (databaseType === 'oracle' && !oracleConnectionString.trim()) {
			errors.connectionString = 'Connection string is required';
		}

		return Object.keys(errors).length === 0;
	}

	async function openExistingDatabase() {
		try {
			const selectedPath = await Commands.pickSqliteDbDialog();
			if (selectedPath) {
				sqliteFilePath = selectedPath;
				if (errors.sqliteFilePath) {
					errors = { ...errors };
					delete errors.sqliteFilePath;
				}
			}
		} catch (error) {
			console.error('Failed to open file dialog:', error);
		}
	}

	async function openExistingSqlcipher() {
		try {
			const selectedPath = await Commands.pickSqlcipherDbDialog();
			if (selectedPath) {
				sqliteSqlcipher = true;
				sqliteFilePath = selectedPath;
				if (errors.sqliteFilePath) {
					errors = { ...errors };
					delete errors.sqliteFilePath;
				}
			}
		} catch (error) {
			console.error('Failed to open SQLCipher file dialog:', error);
		}
	}

	async function openExistingDuckdb() {
		try {
			const selectedPath = await Commands.pickDuckdbDbDialog();
			if (selectedPath) {
				duckdbFilePath = selectedPath;
				if (errors.duckdbFilePath) {
					errors = { ...errors };
					delete errors.duckdbFilePath;
				}
			}
		} catch (error) {
			console.error('Failed to open DuckDB file dialog:', error);
		}
	}

	async function createNewDatabase() {
		try {
			const selectedPath = await Commands.saveSqliteDbDialog();
			if (selectedPath) {
				sqliteFilePath = selectedPath;
				if (errors.sqliteFilePath) {
					errors = { ...errors };
					delete errors.sqliteFilePath;
				}
			}
		} catch (error) {
			console.error('Failed to create database:', error);
		}
	}

	async function createNewDuckdb() {
		try {
			const selectedPath = await Commands.saveDuckdbDbDialog();
			if (selectedPath) {
				duckdbFilePath = selectedPath;
				if (errors.duckdbFilePath) {
					errors = { ...errors };
					delete errors.duckdbFilePath;
				}
			}
		} catch (error) {
			console.error('Failed to create DuckDB database:', error);
		}
	}

	async function selectCaCert() {
		try {
			const selectedPath = await Commands.pickCaCert();
			if (selectedPath) {
				caCertPath = selectedPath;
			}
		} catch (error) {
			console.error('Failed to select CA certificate:', error);
		}
	}

	function clearCaCert() {
		caCertPath = '';
	}

	async function testConnection() {
		if (!validateForm()) return;

		isTestingConnection = true;
		testResult = null;

		const databaseInfo: DatabaseInfo =
			databaseType === 'postgres'
				? {
						Postgres: {
							connection_string: connectionString.trim(),
							ca_cert_path: caCertPath.trim() || null
						}
					}
				: databaseType === 'sqlite'
					? {
							SQLite: {
								db_path: sqliteFilePath.trim(),
								passphrase: sqliteSqlcipher ? sqlitePassphrase.trim() : null
							}
						}
					: databaseType === 'duckdb'
						? { DuckDB: { db_path: duckdbFilePath.trim() } }
						: databaseType === 'mssql'
							? {
									Mssql: {
										connection_string: connectionString.trim(),
										ca_cert_path: caCertPath.trim() || null
									}
								}
							: {
									Oracle: {
										connection_string: oracleConnectionString.trim(),
										wallet_path: oracleWalletPath.trim() || null,
										tns_alias: oracleTnsAlias.trim() || null
									}
								};

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

	async function selectOracleWalletDir() {
		try {
			const selectedPath = await Commands.pickOracleWalletDir();
			if (selectedPath) {
				oracleWalletPath = selectedPath;
			}
		} catch (error) {
			console.error('Failed to select wallet directory:', error);
		}
	}

	function clearOracleWalletDir() {
		oracleWalletPath = '';
	}

	function handleSubmit(e: Event) {
		e.preventDefault();

		if (validateForm()) {
			const databaseInfo: DatabaseInfo =
				databaseType === 'postgres'
					? {
							Postgres: {
								connection_string: connectionString.trim(),
								ca_cert_path: caCertPath.trim() || null
							}
						}
					: databaseType === 'sqlite'
						? {
								SQLite: {
									db_path: sqliteFilePath.trim(),
									passphrase: sqliteSqlcipher ? sqlitePassphrase.trim() : null
								}
							}
						: databaseType === 'duckdb'
							? { DuckDB: { db_path: duckdbFilePath.trim() } }
							: databaseType === 'mssql'
								? {
										Mssql: {
											connection_string: connectionString.trim(),
											ca_cert_path: caCertPath.trim() || null
										}
									}
								: {
										Oracle: {
											connection_string: oracleConnectionString.trim(),
											wallet_path: oracleWalletPath.trim() || null,
											tns_alias: oracleTnsAlias.trim() || null
										}
									};

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
				<Tabs.List class="bg-muted/20 grid w-full grid-cols-5 gap-1 rounded-lg p-1">
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
					<Tabs.Trigger
						value="duckdb"
						class="data-[state=inactive]:hover:bg-muted/30 data-[state=inactive]:text-muted-foreground flex items-center justify-center gap-2 rounded-md px-4 py-2.5 text-sm font-semibold transition-all duration-200 data-[state=active]:bg-[var(--border)] data-[state=active]:shadow-lg"
					>
						<IconSimpleIconsDuckdb class="h-4 w-4" />
						DuckDB
					</Tabs.Trigger>
					<Tabs.Trigger
						value="mssql"
						class="data-[state=inactive]:hover:bg-muted/30 data-[state=inactive]:text-muted-foreground flex items-center justify-center gap-2 rounded-md px-4 py-2.5 text-sm font-semibold transition-all duration-200 data-[state=active]:bg-[var(--border)] data-[state=active]:shadow-lg"
					>
						<IconSimpleIconsMssql class="h-4 w-4" />
						MSSQL
					</Tabs.Trigger>
					<Tabs.Trigger
						value="oracle"
						class="data-[state=inactive]:hover:bg-muted/30 data-[state=inactive]:text-muted-foreground flex items-center justify-center gap-2 rounded-md px-4 py-2.5 text-sm font-semibold transition-all duration-200 data-[state=active]:bg-[var(--border)] data-[state=active]:shadow-lg"
					>
						Oracle
					</Tabs.Trigger>
				</Tabs.List>

				<Tabs.Content value="postgres" class="mt-3">
					<div class="bg-card rounded-xl border p-5 shadow-sm">
						<FormRow
							label="Connection String"
							forId="connectionString"
							required
							error={errors.connectionString}
							size="sm"
							let:describedby
						>
							<Input
								id="connectionString"
								type="text"
								bind:value={connectionString}
								aria-describedby={describedby}
								placeholder="postgresql://username:password@localhost:5432/database"
								class={`shadow-sm transition-shadow focus:shadow-md ${errors.connectionString ? 'border-error focus:ring-error/30' : 'focus:ring-primary/30'}`}
							/>
						</FormRow>

						<div class="mt-4">
							<FormRow
								label="CA Certificate"
								forId="caCertPath"
								description="Provide a custom CA certificate file (.pem, .crt, .cer) for SSL/TLS connections if required."
								size="sm"
								let:describedby
							>
								<div class="space-y-2">
									<div class="flex gap-2">
										<Input
											id="caCertPath"
											type="text"
											bind:value={caCertPath}
											placeholder="No certificate selected..."
											readonly
											aria-describedby={describedby}
											class="flex-1 shadow-sm transition-shadow"
										/>
										<Button
											type="button"
											variant="outline"
											size="sm"
											onclick={selectCaCert}
											class="gap-2 shadow-sm hover:shadow-md"
										>
											<FileCheck class="h-4 w-4" />
											Select
										</Button>
										{#if caCertPath}
											<Button
												type="button"
												variant="ghost"
												size="sm"
												onclick={clearCaCert}
												title="Clear certificate"
												class="px-2"
											>
												<X class="h-4 w-4" />
											</Button>
										{/if}
									</div>
								</div>
							</FormRow>
						</div>
					</div>
				</Tabs.Content>

				<Tabs.Content value="oracle" class="mt-3">
					<div class="bg-card rounded-xl border p-5 shadow-sm">
						<label
							for="oracleConnectionString"
							class="text-foreground mb-2 block text-sm font-semibold"
						>
							Connection String <span class="text-error">*</span>
						</label>
						<Input
							id="oracleConnectionString"
							type="text"
							bind:value={oracleConnectionString}
							placeholder="oracle://user:password@host:1521/service"
							class={`shadow-sm transition-shadow focus:shadow-md ${errors.connectionString ? 'border-error focus:ring-error/30' : 'focus:ring-primary/30'}`}
						/>
						{#if errors.connectionString}
							<p class="text-error mt-2 flex items-center gap-2 text-sm">
								<AlertCircle class="h-4 w-4" />
								{errors.connectionString}
							</p>
						{/if}

						<div class="mt-4 grid grid-cols-2 gap-3">
							<div>
								<FormRow
									label="Wallet Path (TNS_ADMIN)"
									forId="oracleWalletPath"
									description="Select the directory containing Oracle wallet files and sqlnet.ora. This sets TNS_ADMIN for secure connections."
									size="sm"
								>
									<div class="flex gap-2" let:describedby>
										<Input
											id="oracleWalletPath"
											type="text"
											bind:value={oracleWalletPath}
											placeholder="Optional"
											readonly
											aria-describedby={describedby}
											class="flex-1 shadow-sm transition-shadow"
										/>
										<Button
											type="button"
											variant="outline"
											size="sm"
											onclick={selectOracleWalletDir}
											class="gap-2 shadow-sm hover:shadow-md"
										>
											Select
										</Button>
										{#if oracleWalletPath}
											<Button
												type="button"
												variant="ghost"
												size="sm"
												onclick={clearOracleWalletDir}
												title="Clear wallet path"
												class="px-2"
											>
												<X class="h-4 w-4" />
											</Button>
										{/if}
									</div>
								</FormRow>
							</div>
							<div>
								<FormRow label="TNS Alias" forId="oracleTnsAlias" size="sm">
									<Input
										id="oracleTnsAlias"
										type="text"
										bind:value={oracleTnsAlias}
										placeholder="Optional"
										class="shadow-sm transition-shadow"
									/>
								</FormRow>
							</div>
						</div>
					</div>

					{#if editingConnection}
						<div class="mt-4 rounded-lg border p-4">
							<h3 class="mb-3 text-sm font-medium">Oracle Settings (this connection)</h3>
							{#if loadingOracleSettings}
								<div class="text-sm">Loading settings…</div>
							{:else}
								<div class="grid grid-cols-1 gap-3">
									<FormRow label="Oracle XPLAN format" forId="ora_xplan_format" size="sm">
										<select
											id="ora_xplan_format"
											class="w-full rounded border px-2 py-1"
											value={oracleSettings?.xplan_format ?? ''}
											onchange={(e) =>
												updateOracleSetting(
													'xplan_format',
													(e.target as HTMLSelectElement).value as OracleSettings['xplan_format']
												)}
										>
											<option value="TYPICAL">TYPICAL</option>
											<option value="BASIC">BASIC</option>
											<option value="ALL">ALL</option>
											<option value="ALLSTATS LAST">ALLSTATS LAST</option>
										</select>
									</FormRow>
									<FormRow label="XPLAN mode" forId="ora_xplan_mode" size="sm">
										<select
											id="ora_xplan_mode"
											class="w-full rounded border px-2 py-1"
											value={oracleSettings?.xplan_mode ?? ''}
											onchange={(e) =>
												updateOracleSetting(
													'xplan_mode',
													(e.target as HTMLSelectElement).value as OracleSettings['xplan_mode']
												)}
										>
											<option value="display">display</option>
											<option value="display_cursor">display_cursor</option>
										</select>
									</FormRow>
									<FormRow label="RAW format" forId="ora_raw_format" size="sm">
										<select
											id="ora_raw_format"
											class="w-full rounded border px-2 py-1"
											value={oracleSettings?.raw_format ?? ''}
											onchange={(e) =>
												updateOracleSetting(
													'raw_format',
													(e.target as HTMLSelectElement).value as OracleSettings['raw_format']
												)}
										>
											<option value="preview">preview</option>
											<option value="hex">hex</option>
										</select>
									</FormRow>
									<FormRow label="RAW chunk size" forId="ora_raw_chunk_size" size="sm">
										<input
											id="ora_raw_chunk_size"
											class="w-32 rounded border px-2 py-1"
											type="number"
											min="1"
											value={oracleSettings?.raw_chunk_size ?? ''}
											oninput={(e) =>
												updateOracleSetting(
													'raw_chunk_size',
													Number((e.target as HTMLInputElement).value) || undefined
												)}
										/>
									</FormRow>
									<FormRow label="BLOB streaming" forId="ora_blob_stream" size="sm">
										<select
											id="ora_blob_stream"
											class="w-full rounded border px-2 py-1"
											value={oracleSettings?.blob_stream ?? ''}
											onchange={(e) =>
												updateOracleSetting(
													'blob_stream',
													(e.target as HTMLSelectElement).value as OracleSettings['blob_stream']
												)}
										>
											<option value="len">len</option>
											<option value="stream">stream</option>
										</select>
									</FormRow>
									<FormRow label="BLOB chunk size" forId="ora_blob_chunk_size" size="sm">
										<input
											id="ora_blob_chunk_size"
											class="w-32 rounded border px-2 py-1"
											type="number"
											min="1"
											value={oracleSettings?.blob_chunk_size ?? ''}
											oninput={(e) =>
												updateOracleSetting(
													'blob_chunk_size',
													Number((e.target as HTMLInputElement).value) || undefined
												)}
										/>
									</FormRow>

									<FormRow label="Allow DB link ping" forId="ora_allow_db_link_ping" size="sm">
										<input
											id="ora_allow_db_link_ping"
											type="checkbox"
											checked={!!oracleSettings?.allow_db_link_ping}
											onchange={(e) =>
												updateOracleSetting(
													'allow_db_link_ping',
													(e.target as HTMLInputElement).checked
												)}
										/>
									</FormRow>
									<FormRow
										label="Reconnect max retries"
										forId="ora_reconnect_max_retries"
										size="sm"
									>
										<input
											id="ora_reconnect_max_retries"
											class="w-32 rounded border px-2 py-1"
											type="number"
											min="0"
											value={oracleSettings?.reconnect_max_retries ?? ''}
											oninput={(e) =>
												updateOracleSetting(
													'reconnect_max_retries',
													Number((e.target as HTMLInputElement).value) || 0
												)}
										/>
									</FormRow>
									<FormRow
										label="Reconnect backoff (ms)"
										forId="ora_reconnect_backoff_ms"
										size="sm"
									>
										<input
											id="ora_reconnect_backoff_ms"
											class="w-40 rounded border px-2 py-1"
											type="number"
											min="0"
											value={oracleSettings?.reconnect_backoff_ms ?? ''}
											oninput={(e) =>
												updateOracleSetting(
													'reconnect_backoff_ms',
													Number((e.target as HTMLInputElement).value) || 0
												)}
										/>
									</FormRow>
									<FormRow label="Statement cache size" forId="ora_stmt_cache_size" size="sm">
										<input
											id="ora_stmt_cache_size"
											class="w-32 rounded border px-2 py-1"
											type="number"
											min="0"
											value={oracleSettings?.stmt_cache_size ?? ''}
											oninput={(e) =>
												updateOracleSetting(
													'stmt_cache_size',
													Number((e.target as HTMLInputElement).value) || 0
												)}
										/>
									</FormRow>
									<div class="mt-3 flex gap-2">
										<Button
											variant="outline"
											onclick={saveOracleSettings}
											disabled={savingOracleSettings}
										>
											{savingOracleSettings ? 'Saving…' : 'Save Settings'}
										</Button>
									</div>
								</div>
							{/if}
						</div>
					{/if}
				</Tabs.Content>

				<Tabs.Content value="sqlite" class="mt-3">
					<div class="bg-card rounded-xl border p-5 shadow-sm">
						<FormRow
							label="Database File"
							forId="sqliteFilePath"
							required
							error={errors.sqliteFilePath}
							description="Use Open Existing to select an existing SQLite file or Create New to make a new one (.db, .sqlite, .sqlite3)."
							size="sm"
						>
							<div class="space-y-3" let:describedby>
								<Input
									id="sqliteFilePath"
									type="text"
									bind:value={sqliteFilePath}
									placeholder="No database selected..."
									readonly
									aria-describedby={describedby}
									class={`shadow-sm transition-shadow ${errors.sqliteFilePath ? 'border-error' : ''}`}
								/>
								<div class="flex gap-2">
									<Button
										type="button"
										variant="outline"
										onclick={openExistingDatabase}
										class="flex-1 gap-2 shadow-sm hover:shadow-md"
									>
										<FolderOpen class="h-4 w-4" />
										Open Existing
									</Button>
									<Button
										type="button"
										variant="outline"
										onclick={openExistingSqlcipher}
										class="flex-1 gap-2 shadow-sm hover:shadow-md"
									>
										<FolderOpen class="h-4 w-4" />
										Open SQLCipher
									</Button>
									<Button
										type="button"
										variant="outline"
										onclick={createNewDatabase}
										class="flex-1 gap-2 shadow-sm hover:shadow-md"
									>
										<FilePlus class="h-4 w-4" />
										Create New
									</Button>
								</div>
							</div>
						</FormRow>
						<div class="mt-4 grid grid-cols-2 gap-3">
							<div>
								<FormRow label="SQLCipher" forId="sqliteSqlcipher" size="sm">
									<input id="sqliteSqlcipher" type="checkbox" bind:checked={sqliteSqlcipher} />
								</FormRow>
							</div>
							<div>
								<FormRow
									label="Passphrase"
									forId="sqlitePassphrase"
									size="sm"
									error={errors.sqlitePassphrase}
								>
									<Input
										id="sqlitePassphrase"
										type="password"
										bind:value={sqlitePassphrase}
										placeholder="Enter passphrase"
										class={`shadow-sm transition-shadow ${errors.sqlitePassphrase ? 'border-error' : ''}`}
									/>
								</FormRow>
							</div>
						</div>
					</div>
				</Tabs.Content>

				<Tabs.Content value="duckdb" class="mt-3">
					<div class="bg-card rounded-xl border p-5 shadow-sm">
						<FormRow
							label="Database File"
							forId="duckdbFilePath"
							required
							error={errors.duckdbFilePath}
							description="Use Open Existing to select an existing DuckDB file or Create New to make a new one (.duckdb)."
							size="sm"
						>
							<div class="space-y-3" let:describedby>
								<Input
									id="duckdbFilePath"
									type="text"
									bind:value={duckdbFilePath}
									placeholder="No database selected..."
									readonly
									aria-describedby={describedby}
									class={`shadow-sm transition-shadow ${errors.duckdbFilePath ? 'border-error' : ''}`}
								/>
								<div class="flex gap-2">
									<Button
										type="button"
										variant="outline"
										onclick={openExistingDuckdb}
										class="flex-1 gap-2 shadow-sm hover:shadow-md"
									>
										<FolderOpen class="h-4 w-4" />
										Open Existing
									</Button>
									<Button
										type="button"
										variant="outline"
										onclick={createNewDuckdb}
										class="flex-1 gap-2 shadow-sm hover:shadow-md"
									>
										<FilePlus class="h-4 w-4" />
										Create New
									</Button>
								</div>
							</div>
						</FormRow>
					</div>
				</Tabs.Content>

				<Tabs.Content value="mssql" class="mt-3">
					<div class="bg-card rounded-xl border p-5 shadow-sm">
						<FormRow
							label="Connection String"
							forId="mssqlConnectionString"
							required
							error={errors.connectionString}
							size="sm"
							let:describedby
						>
							<Input
								id="mssqlConnectionString"
								type="text"
								bind:value={connectionString}
								aria-describedby={describedby}
								placeholder="sqlserver://user:pass@host:1433;database=dbname"
								class={`shadow-sm transition-shadow focus:shadow-md ${errors.connectionString ? 'border-error focus:ring-error/30' : 'focus:ring-primary/30'}`}
							/>
						</FormRow>

						<div class="mt-4">
							<FormRow
								label="CA Certificate"
								forId="mssqlCaCertPath"
								description="Provide a custom CA certificate (.pem, .crt, .cer) if your server requires a specific CA for TLS."
								size="sm"
								let:describedby
							>
								<div class="space-y-2">
									<div class="flex gap-2">
										<Input
											id="mssqlCaCertPath"
											type="text"
											bind:value={caCertPath}
											placeholder="No certificate selected..."
											readonly
											aria-describedby={describedby}
											class="flex-1 shadow-sm transition-shadow"
										/>
										<Button
											type="button"
											variant="outline"
											size="sm"
											onclick={selectCaCert}
											class="gap-2 shadow-sm hover:shadow-md"
										>
											<FileCheck class="h-4 w-4" />
											Select
										</Button>
										{#if caCertPath}
											<Button
												type="button"
												variant="ghost"
												size="sm"
												onclick={clearCaCert}
												title="Clear certificate"
												class="px-2"
											>
												<X class="h-4 w-4" />
											</Button>
										{/if}
									</div>
								</div>
							</FormRow>
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
