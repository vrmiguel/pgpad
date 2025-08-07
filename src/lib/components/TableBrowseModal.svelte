<script lang="ts">
	import { Table, X } from '@lucide/svelte';
	import { Card, CardHeader, CardTitle, CardContent } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import StreamingQueryResults from './StreamingQueryResults.svelte';

	interface Props {
		isOpen: boolean;
		tableName: string;
		schema: string;
		connectionId: string;
		onClose: () => void;
	}

	let { isOpen, tableName, schema, connectionId, onClose }: Props = $props();

	let currentQuery = $state<string>('');
	let error = $state<string | null>(null);

	$effect(() => {
		if (isOpen && tableName && connectionId) {
			const query =
				schema === 'public'
					? `SELECT * FROM "${tableName}" LIMIT 1000`
					: `SELECT * FROM "${schema}"."${tableName}" LIMIT 1000`;
			currentQuery = query;
			error = null;
		}
	});

	function handleQueryComplete(rowCount: number, duration: number) {
		console.log(`Table browse completed: ${rowCount} rows in ${duration}ms`);
	}

	function handleQueryError(errorMessage: string) {
		error = errorMessage;
		console.error('Table browse failed:', errorMessage);
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onClose();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onClose();
		}
	}
</script>

<svelte:window on:keydown={handleKeydown} />

{#if isOpen}
	<div
		class="bg-background/80 fixed inset-0 z-50 flex items-center justify-center backdrop-blur-sm"
		role="dialog"
		aria-modal="true"
		tabindex="-1"
		onclick={handleBackdropClick}
		onkeydown={handleKeydown}
	>
		<Card class="flex h-[90vh] w-[90vw] flex-col overflow-hidden shadow-xl">
			<CardHeader class="flex-shrink-0 pb-2">
				<div class="flex items-center justify-between">
					<CardTitle class="flex items-center gap-2 text-lg">
						<Table class="h-5 w-5" />
						{schema !== 'public' ? `${schema}.${tableName}` : tableName}
					</CardTitle>
					<Button variant="ghost" size="sm" onclick={onClose}>
						<X class="h-4 w-4" />
					</Button>
				</div>
			</CardHeader>

			<CardContent class="flex min-h-0 flex-1 flex-col">
				{#if currentQuery && connectionId}
					<StreamingQueryResults
						{connectionId}
						query={currentQuery}
						onComplete={handleQueryComplete}
						onError={handleQueryError}
					/>
				{:else}
					<div class="text-muted-foreground flex flex-1 items-center justify-center">
						<div class="text-center">
							<div
								class="bg-muted/20 mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full"
							>
								<Table class="text-muted-foreground/50 h-8 w-8" />
							</div>
							<p class="text-sm font-medium">Select a table to browse</p>
							<p class="text-muted-foreground/70 mt-1 text-xs">Table data will appear here</p>
						</div>
					</div>
				{/if}
			</CardContent>
		</Card>
	</div>
{/if}
