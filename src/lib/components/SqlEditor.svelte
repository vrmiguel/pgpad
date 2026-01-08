<script lang="ts">
	import { ResizablePaneGroup, ResizablePane, ResizableHandle } from '$lib/components/ui/resizable';
	import { Card, CardContent } from '$lib/components/ui/card';
	import QueryResultsView from './QueryResultsView.svelte';
	import { Commands, type ConnectionInfo, type Script } from '$lib/commands.svelte';
	import { createEditor } from '$lib/codemirror';
	import { onMount } from 'svelte';
	import { EditorState } from '@codemirror/state';

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
	let queryKey = $state(0); // Used to trigger re-execution

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

		// Update query and increment key to trigger re-execution
		queryToExecute = query.trim();
		queryKey++;
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
					{#key queryKey}
						<QueryResultsView
							query={queryToExecute}
							connectionId={selectedConnection}
							onQueryComplete={handleQueryComplete}
							showResultTabs={true}
						/>
					{/key}
				{:else}
					<Card
						class="flex h-full flex-col gap-0 overflow-hidden rounded-none border-none pt-0 pb-0"
					>
						<CardContent class="flex h-full min-h-0 flex-1 flex-col overflow-hidden px-6 pt-0">
							<div class="text-muted-foreground flex flex-1 items-center justify-center">
								<div class="space-y-3 text-xs">
									<div class="flex min-w-[20rem] items-center justify-between gap-4">
										<span class="text-muted-foreground/80">Run selected text or current line</span>
										<kbd
											class="bg-muted text-muted-foreground pointer-events-none inline-flex h-5 items-center justify-center gap-1 rounded border px-1.5 font-mono text-[10px] font-medium opacity-100 select-none"
										>
											<span class="text-xs">⌘</span>Enter
										</kbd>
									</div>
									<div class="flex min-w-[20rem] items-center justify-between gap-4">
										<span class="text-muted-foreground/80">Run entire script</span>
										<kbd
											class="bg-muted text-muted-foreground pointer-events-none inline-flex h-5 items-center justify-center gap-1 rounded border px-1.5 font-mono text-[10px] font-medium opacity-100 select-none"
										>
											<span class="text-xs">⌘</span>R
										</kbd>
									</div>
								</div>
							</div>
						</CardContent>
					</Card>
				{/if}
			</div>
		</ResizablePane>
	</ResizablePaneGroup>
</div>
