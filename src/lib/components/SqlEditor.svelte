<script lang="ts">
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Table, FileText, Clock } from '@lucide/svelte';

	interface Props {
		selectedConnection: string | null;
	}

	let { selectedConnection }: Props = $props();
	
	let sqlQuery = $state(`-- Welcome to PgPad!
-- Write your SQL queries here

SELECT 
    table_name,
    column_name,
    data_type
FROM information_schema.columns
WHERE table_schema = 'public'
ORDER BY table_name, ordinal_position;`);

	let results = $state([
		{ table_name: 'users', column_name: 'id', data_type: 'integer' },
		{ table_name: 'users', column_name: 'email', data_type: 'character varying' },
		{ table_name: 'users', column_name: 'created_at', data_type: 'timestamp with time zone' },
		{ table_name: 'posts', column_name: 'id', data_type: 'integer' },
		{ table_name: 'posts', column_name: 'title', data_type: 'text' },
		{ table_name: 'posts', column_name: 'content', data_type: 'text' },
		{ table_name: 'posts', column_name: 'user_id', data_type: 'integer' }
	]);

	let queryHistory = $state([
		{ 
			id: 1, 
			query: "SELECT * FROM users LIMIT 5;", 
			timestamp: "2024-01-15 14:30:22",
			status: "success",
			rows: 5,
			duration: "12ms"
		},
		{
			id: 2,
			query: "SELECT COUNT(*) FROM posts WHERE created_at > NOW() - INTERVAL '7 days';",
			timestamp: "2024-01-15 14:28:15", 
			status: "success",
			rows: 1,
			duration: "8ms"
		},
		{
			id: 3,
			query: "SELECT * FROM non_existent_table;",
			timestamp: "2024-01-15 14:25:10",
			status: "error",
			rows: 0,
			duration: "3ms"
		}
	]);
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