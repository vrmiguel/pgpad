<script lang="ts">
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Table, FileText, Clock } from '@lucide/svelte';
	import { DatabaseCommands, type ConnectionInfo, type QueryResult } from '$lib/commands.svelte';

	interface Props {
		selectedConnection: string | null;
		connections: ConnectionInfo[];
	}

	let { selectedConnection, connections }: Props = $props();
	
	let sqlQuery = $state(`-- Welcome to PgPad!
-- Write your SQL queries here

SELECT 
    table_name,
    column_name,
    data_type
FROM information_schema.columns
WHERE table_schema = 'public'
ORDER BY table_name, ordinal_position;`);

	let queryResult = $state<QueryResult | null>(null);
	let isExecuting = $state(false);
	let queryHistory = $state<Array<{
		id: number;
		query: string;
		timestamp: string;
		status: 'success' | 'error';
		rows: number;
		duration: string;
		error?: string;
	}>>([]);

	export async function handleExecuteQuery() {
		if (!selectedConnection || !sqlQuery.trim()) return;

		isExecuting = true;
		const start = Date.now();

		try {
			const result = await DatabaseCommands.executeQuery(selectedConnection, sqlQuery.trim());
			queryResult = result;
			
			// Add to history
			queryHistory.unshift({
				id: Date.now(),
				query: sqlQuery.trim(),
				timestamp: new Date().toLocaleString(),
				status: 'success',
				rows: result.row_count,
				duration: `${result.duration_ms}ms`
			});
		} catch (error) {
			console.error('Query execution failed:', error);
			
			// Add error to history
			queryHistory.unshift({
				id: Date.now(),
				query: sqlQuery.trim(),
				timestamp: new Date().toLocaleString(),
				status: 'error',
				rows: 0,
				duration: `${Date.now() - start}ms`,
				error: String(error)
			});
		} finally {
			isExecuting = false;
		}
	}



	// Make results reactive based on queryResult
	const results = $derived(queryResult?.rows.map(row => {
		const rowObj: Record<string, any> = {};
		queryResult?.columns.forEach((col, i) => {
			rowObj[col] = row[i];
		});
		return rowObj;
	}) || []);
</script>

<div class="flex-1 flex flex-col p-4 gap-4">
	<!-- SQL Editor -->
	<Card class="flex-1">
		<CardHeader>
			<CardTitle class="flex items-center gap-2">
				<FileText class="w-4 h-4" />
				SQL Editor
			</CardTitle>
		</CardHeader>
		<CardContent class="flex-1 p-0">
			<textarea
				bind:value={sqlQuery}
				class="w-full h-full min-h-[300px] p-4 font-mono text-sm border-0 resize-none focus:outline-none focus:ring-0"
				placeholder="Write your SQL query here..."
				disabled={!selectedConnection}
			></textarea>
		</CardContent>
	</Card>

	<!-- Results Section -->
	<div class="flex gap-4 h-80">
		<!-- Query Results -->
		<Card class="flex-1">
			<CardHeader>
				<CardTitle class="flex items-center gap-2">
					<Table class="w-4 h-4" />
					Results
					{#if results.length > 0}
						<span class="text-sm font-normal text-gray-500">({results.length} rows)</span>
					{/if}
				</CardTitle>
			</CardHeader>
			<CardContent class="p-0">
				<div class="overflow-auto h-60">
					{#if results.length > 0}
						<table class="w-full text-sm">
							<thead class="bg-gray-50 sticky top-0">
								<tr>
									{#each Object.keys(results[0]) as column}
										<th class="text-left p-3 font-medium text-gray-700 border-b">
											{column}
										</th>
									{/each}
								</tr>
							</thead>
							<tbody>
								{#each results as row, i}
									<tr class="border-b hover:bg-gray-50">
										{#each Object.values(row) as value}
											<td class="p-3 text-gray-900">{value}</td>
										{/each}
									</tr>
								{/each}
							</tbody>
						</table>
					{:else}
						<div class="flex items-center justify-center h-60 text-gray-500">
							<div class="text-center">
								<Table class="w-8 h-8 mx-auto mb-2 opacity-50" />
								<p class="text-sm">No results to display</p>
								<p class="text-xs text-gray-400 mt-1">Run a query to see results here</p>
							</div>
						</div>
					{/if}
				</div>
			</CardContent>
		</Card>

		<!-- Query History -->
		<Card class="w-80">
			<CardHeader>
				<CardTitle class="flex items-center gap-2">
					<Clock class="w-4 h-4" />
					Query History
				</CardTitle>
			</CardHeader>
			<CardContent class="p-0">
				<div class="overflow-auto h-60">
					{#if queryHistory.length > 0}
						<div class="space-y-2 p-3">
							{#each queryHistory as query}
								<div class="border rounded-lg p-3 hover:bg-gray-50 cursor-pointer">
									<div class="font-mono text-xs text-gray-600 mb-1 truncate">
										{query.query}
									</div>
									<div class="flex items-center justify-between text-xs text-gray-500">
										<span>{query.timestamp}</span>
										<div class="flex items-center gap-2">
											<span class="px-1.5 py-0.5 rounded text-xs {query.status === 'success' ? 'bg-green-100 text-green-700' : 'bg-red-100 text-red-700'}">
												{query.status}
											</span>
											<span>{query.duration}</span>
										</div>
									</div>
									{#if query.status === 'success'}
										<div class="text-xs text-gray-400 mt-1">
											{query.rows} row{query.rows !== 1 ? 's' : ''}
										</div>
									{/if}
								</div>
							{/each}
						</div>
					{:else}
						<div class="flex items-center justify-center h-60 text-gray-500">
							<div class="text-center">
								<Clock class="w-8 h-8 mx-auto mb-2 opacity-50" />
								<p class="text-sm">No queries yet</p>
								<p class="text-xs text-gray-400 mt-1">Your query history will appear here</p>
							</div>
						</div>
					{/if}
				</div>
			</CardContent>
		</Card>
	</div>
</div> 