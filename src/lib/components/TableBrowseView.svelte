<script lang="ts">
	import QueryResultsView from './QueryResultsView.svelte';

	interface Props {
		tableName: string;
		schema: string;
		connectionId: string;
	}

	let { tableName, schema, connectionId }: Props = $props();

	const query = $derived.by(() => {
		return !schema || schema === 'public'
			? `SELECT * FROM "${tableName}" LIMIT 1000`
			: `SELECT * FROM "${schema}"."${tableName}" LIMIT 1000`;
	});
</script>

<div class="flex h-full flex-col">
	<QueryResultsView {query} {connectionId} showResultTabs={false} />
</div>
