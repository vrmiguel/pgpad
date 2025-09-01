<script lang="ts">
	import { Commands, type QueryStreamEvent, type Row } from '$lib/commands.svelte';
	import { untrack } from 'svelte';

	interface StatementTab {
		id: number;
		status: 'running' | 'completed' | 'error';
		columns?: string[];
		rows?: Row[];
		affectedRows?: number;
		error?: string;
		queryReturnsResults?: boolean;
	}

	interface Props {
		connectionId: string;
		query: string;
		executionTrigger: number;
		onStatementStart?: (
			statementIndex: number,
			statement: string,
			returnsValues: boolean
		) => number;
		onStatementComplete?: (tabId: number, rowCount: number, duration: number) => void;
		onStatementError?: (tabId: number, error: string) => void;
		onTabUpdate?: (tabId: number, updates: Partial<StatementTab>) => void;
		onAllComplete?: () => void;
	}

	let {
		connectionId,
		query,
		executionTrigger,
		onStatementStart,
		onStatementComplete,
		onStatementError,
		onTabUpdate,
		onAllComplete
	}: Props = $props();

	let isExecuting = $state(false);
	let lastExecutionTrigger = $state<number>(-1);
	let startTime = $state<number>(0);

	// Maps statement index to tab ID
	let statementTabMap = $state<Map<number, number>>(new Map());

	function cleanup() {
		statementTabMap.clear();
		isExecuting = false;
	}

	function handleQueryStreamEvent(event: QueryStreamEvent) {
		console.log('StatementExecutor got event:', event);

		switch (event.event) {
			case 'statementStart': {
				console.log('Statement started:', event.data);
				const tabId = onStatementStart?.(
					event.data.statementIndex,
					event.data.statement,
					event.data.returnsValues
				);
				if (tabId) {
					statementTabMap.set(event.data.statementIndex, tabId);
					// Update tab to running status
					onTabUpdate?.(tabId, { status: 'running' });
				}
				break;
			}

			case 'resultStart': {
				console.log('Result started for statement:', event.data.statementIndex);
				const startTabId = statementTabMap.get(event.data.statementIndex);
				if (startTabId) {
					onTabUpdate?.(startTabId, {
						columns: event.data.columns,
						rows: []
					});
				}
				break;
			}

			case 'resultBatch': {
				console.log('Got batch data for statement:', event.data.statementIndex);
				const batchTabId = statementTabMap.get(event.data.statementIndex);
				if (batchTabId) {
					// We need to get current rows and append to them
					// This will be handled by the parent component
					onTabUpdate?.(batchTabId, {
						rows: event.data.rows
					});
				}
				break;
			}

			case 'statementComplete': {
				const completeTabId = statementTabMap.get(event.data.statementIndex);
				if (completeTabId) {
					onTabUpdate?.(completeTabId, {
						status: 'completed',
						affectedRows: event.data.affectedRows
					});
					onStatementComplete?.(completeTabId, event.data.affectedRows, Date.now() - startTime);
				}
				break;
			}

			case 'statementFinish': {
				const finishTabId = statementTabMap.get(event.data.statementIndex);
				if (finishTabId) {
					onTabUpdate?.(finishTabId, { status: 'completed' });
				}
				break;
			}

			case 'allFinished': {
				console.log('All statements finished');
				cleanup();
				onAllComplete?.();
				break;
			}

			case 'statementError': {
				console.error('Statement error:', event.data);
				const errorTabId = statementTabMap.get(event.data.statementIndex);
				if (errorTabId) {
					onTabUpdate?.(errorTabId, {
						status: 'error',
						error: event.data.error
					});
					onStatementError?.(errorTabId, event.data.error);
				}
				break;
			}
		}
	}

	async function executeQuery() {
		if (isExecuting) return;

		console.log('StatementExecutor starting execution');
		isExecuting = true;
		startTime = Date.now();

		try {
			await Commands.executeQueryStream(connectionId, query, handleQueryStreamEvent);
		} catch (error) {
			console.error('Query execution failed:', error);
			cleanup();
		}
	}

	$effect(() => {
		if (connectionId && query && executionTrigger !== lastExecutionTrigger) {
			console.log('New execution triggered:', query, 'trigger:', executionTrigger);
			lastExecutionTrigger = executionTrigger;

			cleanup();

			untrack(() => {
				if (!isExecuting) {
					executeQuery();
				}
			});
		}
	});
</script>

<!-- Component without UI -->
