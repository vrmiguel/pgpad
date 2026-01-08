<script lang="ts">
	import { onDestroy } from 'svelte';
	import { Card, CardContent } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { ChevronLeft, ChevronRight } from '@lucide/svelte';
	import Table from './Table.svelte';
	import JsonInspector from './JsonInspector.svelte';
	import TabBar from '$lib/components/ui/TabBar.svelte';
	import KeyboardShortcuts from './KeyboardShortcuts.svelte';
	import { QueryExecutor } from '$lib/queryExecutor.svelte';
	import type { Json } from '$lib/commands.svelte';

	interface Props {
		/** The SQL query to execute */
		query: string;
		/** Connection ID to execute against */
		connectionId: string;
		/** Callback when query completes successfully */
		onQueryComplete?: (totalRows: number) => void;
		/** Whether to show tabs. Useful for table-view so that it doesn't show the tabs component and takes up the whole space */
		showResultTabs?: boolean;
	}

	let { query, connectionId, onQueryComplete, showResultTabs = true }: Props = $props();

	let selectedCellData = $state<Json | null>(null);
	let jsonInspectorData = $state<{ data: Json; position: { x: number; y: number } } | null>(null);

	const executor = $state(new QueryExecutor());

	// This is not meant to truly and accurately measure any query's execution time, it's just for the
	// UI to not "flicker" if executing a very fast query.
	// Without something like this, a query that finishes in a few milliseconds would very briefly flash
	// "Loading results..." or etc, only to then be overwritten with the Table with the results.
	let lastQueryStartedAt = $state(performance.now());

	$effect(() => {
		lastQueryStartedAt = performance.now();
		executor.executeQuery(query, connectionId, onQueryComplete);
	});

	onDestroy(() => {
		executor.dispose();
	});
</script>

<div class="relative flex h-full flex-col">
	{#if showResultTabs && executor.resultTabs.length > 0}
		<div class="relative z-10">
			<TabBar
				tabs={executor.resultTabs}
				activeTabId={executor.activeResultTabId}
				onTabSelect={executor.handleResultTabSelect}
				onTabClose={executor.handleResultTabClose}
				onNewTab={undefined}
				onTabRename={undefined}
				showCloseButton={true}
				showNewTabButton={false}
				allowRename={false}
				getTabStatus={executor.getTabStatus}
				closeTabLabel="Close result tab"
				maxTabWidth="max-w-64"
				variant="default"
			/>
		</div>
	{/if}

	<Card
		class="flex flex-1 flex-col gap-0 overflow-hidden rounded-none border-none {showResultTabs
			? 'pt-0'
			: ''} pb-0"
	>
		{#if executor.resultTabs.length > 0 && executor.activeResultTabId}
			{@const activeTab = executor.resultTabs.find((t) => t.id === executor.activeResultTabId)}
			{#if activeTab}
				{#if activeTab.columns && activeTab.currentPageData && activeTab.currentPageData.length > 0}
					<div class="relative flex min-h-0 flex-1 flex-col">
						<CardContent class="flex h-full min-h-0 flex-1 flex-col overflow-hidden p-0">
							<Table
								data={activeTab.currentPageData}
								columns={activeTab.columns}
								bind:selectedCellData
								onJsonInspect={(data, position) => {
									jsonInspectorData = { data, position };
								}}
							/>
						</CardContent>

						{#if activeTab.totalPages && activeTab.totalPages > 1}
							<div
								class="border-border/30 bg-muted/20 flex flex-shrink-0 items-center border-t px-3 py-2"
							>
								<div class="text-muted-foreground flex items-center gap-2 text-xs">
									<span>Page {activeTab.currentPageIndex + 1} of {activeTab.totalPages}</span>
								</div>

								<div class="flex-1"></div>

								<div class="flex items-center gap-1">
									<Button
										variant="ghost"
										size="sm"
										onclick={() =>
											executor.loadPage(activeTab.queryId, activeTab.currentPageIndex - 1)}
										disabled={activeTab.currentPageIndex === 0}
										class="h-6 w-6 p-0"
									>
										<ChevronLeft class="h-3 w-3" />
									</Button>

									{#if activeTab.currentPageIndex > 1}
										<Button
											variant="ghost"
											size="sm"
											onclick={() => executor.loadPage(activeTab.queryId, 0)}
											class="h-6 px-2 text-xs"
										>
											1
										</Button>
										{#if activeTab.currentPageIndex > 2}
											<span class="text-muted-foreground text-xs">...</span>
										{/if}
									{/if}

									{#if activeTab.currentPageIndex > 0}
										<Button
											variant="ghost"
											size="sm"
											onclick={() =>
												executor.loadPage(activeTab.queryId, activeTab.currentPageIndex - 1)}
											class="h-6 px-2 text-xs"
										>
											{activeTab.currentPageIndex}
										</Button>
									{/if}

									<Button variant="default" size="sm" class="h-6 px-2 text-xs">
										{activeTab.currentPageIndex + 1}
									</Button>

									{#if activeTab.currentPageIndex < activeTab.totalPages - 1}
										<Button
											variant="ghost"
											size="sm"
											onclick={() =>
												executor.loadPage(activeTab.queryId, activeTab.currentPageIndex + 1)}
											class="h-6 px-2 text-xs"
										>
											{activeTab.currentPageIndex + 2}
										</Button>
									{/if}

									{#if activeTab.totalPages && activeTab.currentPageIndex < activeTab.totalPages - 2}
										{#if activeTab.currentPageIndex < activeTab.totalPages - 3}
											<span class="text-muted-foreground text-xs">...</span>
										{/if}
										<Button
											variant="ghost"
											size="sm"
											onclick={() =>
												activeTab.totalPages &&
												executor.loadPage(activeTab.queryId, activeTab.totalPages - 1)}
											class="h-6 px-2 text-xs"
										>
											{activeTab.totalPages}
										</Button>
									{/if}

									<Button
										variant="ghost"
										size="sm"
										onclick={() =>
											executor.loadPage(activeTab.queryId, activeTab.currentPageIndex + 1)}
										disabled={activeTab.currentPageIndex >= activeTab.totalPages - 1}
										class="h-6 w-6 p-0"
									>
										<ChevronRight class="h-3 w-3" />
									</Button>
								</div>
							</div>
						{/if}

						{#if jsonInspectorData}
							<JsonInspector
								selectedCellData={jsonInspectorData.data}
								initialPosition={jsonInspectorData.position}
								onClose={() => {
									jsonInspectorData = null;
								}}
							/>
						{/if}
					</div>
				{:else}
					<CardContent class="flex h-full min-h-0 flex-1 flex-col overflow-hidden px-6">
						{#if activeTab.error}
							<div class="flex h-full flex-1 items-center justify-center">
								<div class="text-center">
									<div class="text-sm text-red-600">{activeTab.error}</div>
								</div>
							</div>
						{:else if activeTab.queryReturnsResults === false}
							<div class="flex h-full flex-1 items-center justify-center">
								<div class="text-center">
									{#if activeTab.status === 'Running' && performance.now() - lastQueryStartedAt > 150}
										<div class="text-muted-foreground text-sm">Executing query...</div>
									{:else if activeTab.status === 'Completed'}
										<div class="text-sm font-medium text-green-600">
											✓ {activeTab.affectedRows || 0} rows affected
										</div>
									{/if}
								</div>
							</div>
						{:else if activeTab.status === 'Running' && performance.now() - lastQueryStartedAt > 150}
							<div class="flex h-full flex-1 items-center justify-center">
								<div class="text-center">
									<div class="text-muted-foreground text-sm">Loading results...</div>
								</div>
							</div>
						{:else if activeTab.columns && activeTab.status === 'Completed' && (!activeTab.currentPageData || activeTab.currentPageData.length === 0)}
							<div class="flex h-full flex-1 items-center justify-center">
								<div class="text-center">
									<div class="text-muted-foreground text-sm">No rows returned.</div>
								</div>
							</div>
						{/if}
					</CardContent>
				{/if}
			{/if}
		{:else}
			<!-- We get here if results were shown but then the user closed all tabs -->
			<CardContent class="flex h-full min-h-0 flex-1 flex-col overflow-hidden px-6 pt-0">
				<KeyboardShortcuts />
			</CardContent>
		{/if}
	</Card>
</div>
