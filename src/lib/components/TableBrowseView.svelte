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

<div class="h-full flex-1" style="height: 0;">
	<QueryResultsView {query} {connectionId} showResultTabs={false} />
</div>
