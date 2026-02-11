<script lang="ts">
	import { ResizablePaneGroup, ResizablePane, ResizableHandle } from '$lib/components/ui/resizable';
	import { Card, CardContent } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import QueryResultsView from './QueryResultsView.svelte';
	import KeyboardShortcuts from './KeyboardShortcuts.svelte';
	import { Commands, type ConnectionInfo, type Script } from '$lib/commands.svelte';
	import { createEditor } from '$lib/codemirror';
	import { onMount } from 'svelte';
	import { EditorState } from '@codemirror/state';
	import { AlertDialog } from 'bits-ui';
	import { AlertTriangle, ShieldX } from '@lucide/svelte';

	interface Props {
		selectedConnection: string | null;
		connections: ConnectionInfo[];
		currentScript: Script | null;
		hasUnsavedChanges: boolean;
		onContentChange?: (content: string) => void;
		onLoadFromHistory?: (historyQuery: string) => void;
		onHistoryUpdate?: () => void;
	}

	let {
		selectedConnection = $bindable(),
		connections = $bindable(),
		currentScript = $bindable(),
		hasUnsavedChanges = $bindable(),
		onContentChange,
		onLoadFromHistory,
		onHistoryUpdate
	}: Props = $props();

	let editorContainer = $state<HTMLElement>();
	let sqlEditor: ReturnType<typeof createEditor> | null = null;

	let sqlQuery = $state('');
	let queryToExecute = $state<string>('');
	let executionTrigger = $state(0);
	let showWriteConfirmDialog = $state(false);
	let showReadOnlyBlockedDialog = $state(false);
	let pendingQuery = $state<string>('');

	const isConnected = $derived.by(() => {
		if (!selectedConnection) return false;
		const connection = connections.find((c) => c.id === selectedConnection);
		return connection?.connected || false;
	});

	export function getContent(): string {
		return sqlQuery;
	}

	export function setContent(content: string) {
		sqlQuery = content;
		onContentChange?.(content);
		if (sqlEditor) {
			sqlEditor.updateValue(content);
		}
	}

	// Set content without onContentChange
	export function setContentSilently(content: string) {
		sqlQuery = content;
		if (sqlEditor) {
			sqlEditor.updateValue(content);
		}
	}

	export function saveState(): EditorState | undefined {
		return sqlEditor?.saveState();
	}

	export function restoreState(state: EditorState) {
		if (sqlEditor && state) {
			sqlEditor.restoreState(state);
			sqlQuery = sqlEditor.view.state.doc.toString();
			sqlEditor.syncFontSize();
			onContentChange?.(sqlQuery);
		}
	}

	export function syncFontSize() {
		if (sqlEditor) {
			sqlEditor.syncFontSize();
		}
	}

	export async function handleExecuteQuery(queryText?: string) {
		const query = queryText || sqlQuery;
		if (!selectedConnection || !query.trim()) return;

		if (!isConnected) {
			console.warn('Cannot execute query: No active database connection');
			return;
		}

		const connection = connections.find((c) => c.id === selectedConnection);
		if (!connection) return;

		if (connection.permissions !== 'read_write') {
			try {
				const isReadOnly = await Commands.isQueryReadOnly(selectedConnection, query.trim());

				if (!isReadOnly) {
					if (connection.permissions === 'read_only') {
						// Block execution on read-only connections
						showReadOnlyBlockedDialog = true;
						return;
					} else if (connection.permissions === 'protected_write') {
						pendingQuery = query.trim();
						showWriteConfirmDialog = true;
						return;
					}
				}
			} catch (error) {
				console.error('Failed to check query permissions:', error);
				alert('Failed to validate query permissions. Please try again.');
				return;
			}
		}

		executeQuery(query.trim());
	}

	function executeQuery(query: string) {
		queryToExecute = query;
		executionTrigger++;
	}

	function handleConfirmWrite() {
		showWriteConfirmDialog = false;
		if (pendingQuery) {
			executeQuery(pendingQuery);
			pendingQuery = '';
		}
	}

	function handleCancelWrite() {
		showWriteConfirmDialog = false;
		pendingQuery = '';
	}

	function handleCloseReadOnlyDialog() {
		showReadOnlyBlockedDialog = false;
	}

	export function loadQueryFromHistory(historyQuery: string) {
		if (onLoadFromHistory) {
			onLoadFromHistory(historyQuery);
		}
	}

	function handleQueryComplete(totalRows: number) {
		if (selectedConnection) {
			Commands.saveQueryToHistory(
				selectedConnection,
				queryToExecute,
				undefined,
				'success',
				totalRows,
				undefined
			);
			onHistoryUpdate?.();
		}
	}

	async function loadDatabaseSchema() {
		if (!selectedConnection || !sqlEditor) return;

		try {
			const connection = connections.find((c) => c.id === selectedConnection);
			if (connection?.connected) {
				// Get schema information for autocomplete
				const schema = await Commands.getDatabaseSchema(selectedConnection);
				sqlEditor.updateSchema(schema);
			}
		} catch (error) {
			console.error('Failed to load database schema:', error);
		}
	}

	$effect(() => {
		if (selectedConnection) {
			loadDatabaseSchema();
		}
	});

	onMount(() => {
		const initializeEditor = () => {
			if (editorContainer && editorContainer.offsetParent !== null) {
				sqlEditor = createEditor({
					container: editorContainer,
					value: sqlQuery,
					onChange: (newValue) => {
						sqlQuery = newValue;
						onContentChange?.(newValue);
					},
					onExecute: handleExecuteQuery,
					onExecuteSelection: (selectedText: string) => {
						handleExecuteQuery(selectedText);
					},
					disabled: false,
					schema: null
				});

				loadDatabaseSchema();
			} else {
				setTimeout(initializeEditor, 100);
			}
		};

		initializeEditor();
	});
</script>

<div class="flex flex-1 flex-col">
	<ResizablePaneGroup direction="vertical" class="flex-1">
		<ResizablePane defaultSize={60} minSize={30} maxSize={80}>
			<div class="h-full">
				<Card class="flex h-full flex-col gap-0 overflow-hidden rounded-none border-none py-0">
					<CardContent class="min-h-0 flex-1 p-0">
						<div bind:this={editorContainer} class="h-full w-full"></div>
					</CardContent>
				</Card>
			</div>
		</ResizablePane>

		<ResizableHandle />

		<ResizablePane defaultSize={40} minSize={20}>
			<div class="h-full">
				{#if selectedConnection && queryToExecute}
					<QueryResultsView
						query={queryToExecute}
						connectionId={selectedConnection}
						{executionTrigger}
						onQueryComplete={handleQueryComplete}
						showResultTabs={true}
					/>
				{:else}
					<Card
						class="flex h-full flex-col gap-0 overflow-hidden rounded-none border-none pt-0 pb-0"
					>
						<CardContent class="flex h-full min-h-0 flex-1 flex-col overflow-hidden px-6 pt-0">
							<KeyboardShortcuts />
						</CardContent>
					</Card>
				{/if}
			</div>
		</ResizablePane>
	</ResizablePaneGroup>
</div>

<AlertDialog.Root bind:open={showWriteConfirmDialog}>
	<AlertDialog.Portal>
		<AlertDialog.Overlay
			class="bg-background/80 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 fixed inset-0 z-50 backdrop-blur-sm"
		/>
		<AlertDialog.Content
			class="border-border bg-card data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[state=closed]:slide-out-to-left-1/2 data-[state=closed]:slide-out-to-top-[48%] data-[state=open]:slide-in-from-left-1/2 data-[state=open]:slide-in-from-top-[48%] fixed top-[50%] left-[50%] z-50 grid w-full max-w-lg translate-x-[-50%] translate-y-[-50%] gap-4 rounded-lg border p-6 shadow-lg duration-200"
		>
			<div class="flex items-start gap-3">
				<div class="bg-warning/10 text-warning rounded-full p-2">
					<AlertTriangle class="h-5 w-5" />
				</div>
				<div class="flex-1">
					<AlertDialog.Title class="text-foreground text-lg font-semibold">
						Confirm write operation
					</AlertDialog.Title>
					<AlertDialog.Description class="text-muted-foreground mt-2 text-sm">
						This query contains operations that will modify the database. Are you sure you want to
						proceed?
					</AlertDialog.Description>
				</div>
			</div>

			<div class="flex justify-end gap-2">
				<AlertDialog.Cancel>
					<Button variant="outline" onclick={handleCancelWrite}>Cancel</Button>
				</AlertDialog.Cancel>
				<AlertDialog.Action>
					<Button variant="destructive" onclick={handleConfirmWrite}>Execute Query</Button>
				</AlertDialog.Action>
			</div>
		</AlertDialog.Content>
	</AlertDialog.Portal>
</AlertDialog.Root>

<AlertDialog.Root bind:open={showReadOnlyBlockedDialog}>
	<AlertDialog.Portal>
		<AlertDialog.Overlay
			class="bg-background/80 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 fixed inset-0 z-50 backdrop-blur-sm"
		/>
		<AlertDialog.Content
			class="border-border bg-card data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[state=closed]:slide-out-to-left-1/2 data-[state=closed]:slide-out-to-top-[48%] data-[state=open]:slide-in-from-left-1/2 data-[state=open]:slide-in-from-top-[48%] fixed top-[50%] left-[50%] z-50 grid w-full max-w-lg translate-x-[-50%] translate-y-[-50%] gap-4 rounded-lg border p-6 shadow-lg duration-200"
		>
			<div class="flex items-start gap-3">
				<div class="bg-destructive/10 text-destructive rounded-full p-2">
					<ShieldX class="h-5 w-5" />
				</div>
				<div class="flex-1">
					<AlertDialog.Title class="text-foreground text-lg font-semibold">
						Write operation blocked
					</AlertDialog.Title>
					<AlertDialog.Description class="text-muted-foreground mt-2 text-sm">
						This connection is configured as read-only to prevent accidental modifications. Write
						operations cannot be executed.
					</AlertDialog.Description>
				</div>
			</div>

			<div class="flex justify-end">
				<AlertDialog.Action>
					<Button onclick={handleCloseReadOnlyDialog}>Okay</Button>
				</AlertDialog.Action>
			</div>
		</AlertDialog.Content>
	</AlertDialog.Portal>
</AlertDialog.Root>
