<script lang="ts">
	import { Commands } from '$lib/commands.svelte';

	interface Props {
		selectedConnection: string | null;
	}

	let { selectedConnection }: Props = $props();
	let loading = $state(false);
	let page = $state(1);
	let limit = $state(50);
	let indexName = $state('');
	let tableName = $state('');
	let totalCount = $state(0);
	let indexes: Array<{
		index_name: string;
		table_name: string;
		column_names: string[];
		index_type: string;
		uniqueness: string;
		status: string;
		created: string;
		size_bytes: number;
	}> = $state([]);

	async function load() {
		if (!selectedConnection) return;
		loading = true;
		try {
			const raw = await Commands.getOracleIndexes(
				selectedConnection,
				tableName || undefined,
				indexName || undefined,
				page,
				limit
			);
			const parsed = JSON.parse(raw || '{}');
			indexes = parsed.indexes || [];
			totalCount = parsed.total_count || 0;
		} catch {
			indexes = [];
			totalCount = 0;
		} finally {
			loading = false;
		}
	}

	function handleSearch() {
		page = 1;
		load();
	}

	function nextPage() {
		const maxPage = Math.max(1, Math.ceil(totalCount / limit));
		if (page < maxPage) {
			page += 1;
			load();
		}
	}

	function prevPage() {
		if (page > 1) {
			page -= 1;
			load();
		}
	}

	$effect(() => {
		const c = selectedConnection;
		if (c) {
			load();
		} else {
			indexes = [];
			totalCount = 0;
		}
	});
</script>

<div class="flex h-full flex-col gap-3">
	<div class="flex items-end gap-2 p-2">
		<div class="flex flex-col">
			<label class="text-muted-foreground text-xs">Table Name</label>
			<input
				class="bg-background rounded-sm border px-2 py-1 text-sm"
				bind:value={tableName}
				placeholder="USERS"
			/>
		</div>
		<div class="flex flex-col">
			<label class="text-muted-foreground text-xs">Index Name</label>
			<input
				class="bg-background rounded-sm border px-2 py-1 text-sm"
				bind:value={indexName}
				placeholder="IDX_*"
			/>
		</div>
		<button
			class="enabled:hover:bg-accent rounded-sm border px-3 py-1 text-sm disabled:opacity-50"
			onclick={handleSearch}
			disabled={loading}>Search</button
		>
		<div class="text-muted-foreground ml-auto text-xs">{totalCount} total</div>
	</div>

	<div class="min-h-0 flex-1 overflow-auto">
		{#if loading}
			<div class="p-4 text-sm">Loading...</div>
		{:else}
			<table class="w-full text-sm">
				<thead>
					<tr class="text-left">
						<th class="p-2">Index</th>
						<th class="p-2">Table</th>
						<th class="p-2">Columns</th>
						<th class="p-2">Type</th>
						<th class="p-2">Unique</th>
						<th class="p-2">Status</th>
						<th class="p-2">Created</th>
						<th class="p-2">Size (bytes)</th>
					</tr>
				</thead>
				<tbody>
					{#each indexes as it (it.index_name + ':' + it.table_name)}
						<tr class="border-t">
							<td class="p-2">{it.index_name}</td>
							<td class="p-2">{it.table_name}</td>
							<td class="p-2">{it.column_names?.join(', ')}</td>
							<td class="p-2">{it.index_type}</td>
							<td class="p-2">{it.uniqueness}</td>
							<td class="p-2">{it.status}</td>
							<td class="p-2">{it.created}</td>
							<td class="p-2">{it.size_bytes}</td>
						</tr>
					{/each}
					{#if indexes.length === 0}
						<tr>
							<td class="text-muted-foreground p-4" colspan="8">No indexes</td>
						</tr>
					{/if}
				</tbody>
			</table>
		{/if}
	</div>

	<div class="flex items-center gap-2 p-2">
		<button
			class="enabled:hover:bg-accent rounded-sm border px-3 py-1 text-sm disabled:opacity-50"
			onclick={prevPage}
			disabled={loading || page <= 1}>Prev</button
		>
		<span class="text-xs">Page {page}</span>
		<button
			class="enabled:hover:bg-accent rounded-sm border px-3 py-1 text-sm disabled:opacity-50"
			onclick={nextPage}
			disabled={loading || page >= Math.max(1, Math.ceil(totalCount / limit))}>Next</button
		>
		<div class="ml-auto flex items-center gap-2">
			<label class="text-xs">Page Size</label>
			<select
				class="bg-background rounded-sm border px-2 py-1 text-sm"
				bind:value={limit}
				onchange={() => {
					page = 1;
					load();
				}}
			>
				<option value={25}>25</option>
				<option value={50}>50</option>
				<option value={100}>100</option>
			</select>
		</div>
	</div>
</div>
