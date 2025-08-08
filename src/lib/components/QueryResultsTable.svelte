<script lang="ts">
	import { FlexRender, createSvelteTable, createQueryColumns } from '$lib/components/ui/data-table';
	import type { PgRow } from '$lib/commands.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import JsonViewer from './JsonViewer.svelte';
	import {
		ChevronUp,
		ChevronDown,
		ChevronsUpDown,
		ChevronLeft,
		ChevronRight,
		Search
	} from '@lucide/svelte';
	import { untrack } from 'svelte';

	interface Props {
		data: PgRow[];
		columns: string[];
		table?: any;
		globalFilter?: string;
	}

	let { data, columns, table = $bindable(), globalFilter = $bindable('') }: Props = $props();

	let isResizing = $state(false);
	let resizePreviewX = $state(0);
	let resizingColumnId = $state<string | null>(null);
	let tableContainer: HTMLDivElement;

	const columnDefs = $derived(createQueryColumns(columns));

	const options = {
		get data() {
			console.log('Data getter called, returning', data.length, 'rows');
			return data;
		},
		get columns() {
			console.log('Columns getter called, returning', columnDefs.length, 'columns');
			return columnDefs;
		},
		initialState: {
			pagination: {
				pageIndex: 0,
				pageSize: 50
			},
			globalFilter
		},
		enablePagination: true,
		enableSorting: true,
		enableFiltering: true,
		enableColumnResizing: true
	};

	const tableInstance = createSvelteTable(options);

	$effect(() => {
		if (tableInstance && tableInstance.state.globalFilter !== globalFilter) {
			tableInstance.state.globalFilter = globalFilter;
		}
	});

	$effect(() => {
		if (tableInstance && globalFilter !== tableInstance.state.globalFilter) {
			globalFilter = tableInstance.state.globalFilter;
		}
	});

	$effect(() => {
		table = tableInstance;
	});

	function createResizeHandler(header: any) {
		return (downEvent: MouseEvent | TouchEvent) => {
			if (!tableContainer) return;

			const startX = 'clientX' in downEvent ? downEvent.clientX : downEvent.touches[0].clientX;
			const columnId = header.column.id;
			const containerRect = tableContainer.getBoundingClientRect();

			isResizing = true;
			resizingColumnId = columnId;
			resizePreviewX = startX - containerRect.left + tableContainer.scrollLeft;

			const handleMouseMove = (moveEvent: MouseEvent | TouchEvent) => {
				const currentX = 'clientX' in moveEvent ? moveEvent.clientX : moveEvent.touches[0].clientX;
				resizePreviewX = currentX - containerRect.left + tableContainer.scrollLeft;
			};

			const handleMouseUp = () => {
				isResizing = false;
				resizingColumnId = null;

				document.removeEventListener('mousemove', handleMouseMove);
				document.removeEventListener('mouseup', handleMouseUp);
				document.removeEventListener('touchmove', handleMouseMove);
				document.removeEventListener('touchend', handleMouseUp);
			};

			document.addEventListener('mousemove', handleMouseMove);
			document.addEventListener('mouseup', handleMouseUp);
			document.addEventListener('touchmove', handleMouseMove);
			document.addEventListener('touchend', handleMouseUp);

			const originalHandler = header.getResizeHandler();
			if (originalHandler) {
				originalHandler(downEvent);
			}
		};
	}
</script>

<div class="relative grid h-full grid-rows-[1fr_auto]">
	<!-- Resize preview line -->
	{#if isResizing}
		<div
			class="pointer-events-none absolute top-0 bottom-0 z-20 w-0.5 bg-blue-500 opacity-75"
			style="left: {resizePreviewX}px"
		></div>
	{/if}

	<!-- Table content - takes remaining space -->
	<div class="overflow-hidden" bind:this={tableContainer}>
		<div class="h-full overflow-auto">
			{#if tableInstance}
				<table
					class="w-full border-collapse text-xs"
					style="width: {tableInstance.getCenterTotalSize()}px"
				>
					<thead class="bg-muted/90 border-border sticky top-0 z-10 border-b">
						{#each tableInstance.getHeaderGroups() as headerGroup}
							<tr>
								{#each headerGroup.headers as header}
									<th
										class="text-foreground bg-muted/95 border-border/40 relative border-r px-2 py-1 text-left align-middle text-xs font-medium"
										style="width: {header.getSize()}px"
									>
										{#if !header.isPlaceholder}
											<div class="flex items-center justify-between">
												<Button
													variant="ghost"
													size="sm"
													class="hover:bg-accent/30 -ml-1 h-6 flex-1 justify-start p-1 text-xs font-medium"
													onclick={() =>
														header.column.toggleSorting(header.column.getIsSorted() === 'asc')}
												>
													<FlexRender
														content={header.column.columnDef.header}
														context={header.getContext()}
													/>
													{#if header.column.getIsSorted() === 'asc'}
														<ChevronUp class="text-muted-foreground ml-1 h-3 w-3" />
													{:else if header.column.getIsSorted() === 'desc'}
														<ChevronDown class="text-muted-foreground ml-1 h-3 w-3" />
													{:else}
														<ChevronsUpDown class="text-muted-foreground/40 ml-1 h-3 w-3" />
													{/if}
												</Button>
												<!-- Column Resize Handle with improved visual feedback -->
												{#if header.column.getCanResize()}
													<div
														class="absolute top-0 right-0 h-full w-1 cursor-col-resize bg-transparent transition-colors select-none hover:bg-blue-400 active:bg-blue-500 {resizingColumnId ===
														header.column.id
															? 'bg-blue-500'
															: ''}"
														onmousedown={createResizeHandler(header)}
														ontouchstart={createResizeHandler(header)}
														role="separator"
														aria-label="Resize column"
													></div>
												{/if}
											</div>
										{/if}
									</th>
								{/each}
							</tr>
						{/each}
					</thead>
					<tbody>
						{#each tableInstance.getPaginationRowModel().rows as row (row.id)}
							<tr class="border-border/30 hover:bg-muted/20 border-b transition-colors">
								{#each row.getVisibleCells() as cell (cell.column.id)}
									{@const cellValue = cell.getValue()}
									{@const columnWidth = cell.column.getSize()}
									{@const isObject = typeof cellValue === 'object' && cellValue !== null}
									<td
										class="border-border/20 border-r px-2 py-1 align-top text-xs"
										style="width: {columnWidth}px"
										title={cellValue !== null && cellValue !== undefined
											? (isObject ? '{ .. }' : String(cellValue))
											: undefined}
									>
										{#if cellValue === null || cellValue === undefined}
											<span class="text-muted-foreground text-xs italic">NULL</span>
										{:else if typeof cellValue === 'boolean'}
											<span class="text-foreground">{cellValue ? 'true' : 'false'}</span>
										{:else if typeof cellValue === 'number'}
											<span class="text-foreground font-mono">{cellValue.toLocaleString()}</span>
										{:else if isObject}
											<JsonViewer json={cellValue} depth={2} />
										{:else}
											<div class="text-foreground truncate" style="max-width: {columnWidth - 16}px">
												{String(cellValue || '')}
											</div>
										{/if}
									</td>
								{/each}
							</tr>
						{:else}
							<tr>
								<td
									colspan={columns.length}
									class="h-24 text-center text-muted-foreground border-r border-border/20"
								>
									<div class="flex flex-col items-center gap-2">
										<div class="w-8 h-8 rounded bg-muted/20 flex items-center justify-center">
											<Search class="w-4 h-4 text-muted-foreground/50" />
										</div>
										<div>
											<p class="text-xs font-medium">No results found</p>
											<p class="text-xs text-muted-foreground/70">
												Try adjusting your search terms
											</p>
										</div>
									</div>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{:else}
				<div class="text-muted-foreground flex h-full items-center justify-center">
					<div class="text-center">
						<div class="text-sm">Initializing table...</div>
					</div>
				</div>
			{/if}
		</div>
	</div>

	<!-- Pagination at bottom -->
	{#if tableInstance && tableInstance.getPageCount() > 1}
		<div class="border-border/30 bg-muted/20 flex flex-shrink-0 items-center border-t px-3 py-2">
			<!-- Compact left section -->
			<div class="text-muted-foreground flex items-center gap-2 text-xs">
				<span
					>Page {tableInstance.getState().pagination.pageIndex + 1} of {tableInstance.getPageCount()}</span
				>
				<span>•</span>
				<select
					bind:value={tableInstance.state.pagination.pageSize}
					class="border-border bg-background focus:ring-ring h-6 w-12 rounded border text-xs focus:ring-1"
				>
					{#each [25, 50, 100, 200] as pageSize}
						<option value={pageSize}>{pageSize}</option>
					{/each}
				</select>
				<span>rows</span>
			</div>

			<!-- Spacer -->
			<div class="flex-1"></div>

			<!-- Compact navigation -->
			<div class="flex items-center gap-1">
				<Button
					variant="ghost"
					size="sm"
					onclick={() => tableInstance?.previousPage()}
					disabled={!tableInstance?.getCanPreviousPage()}
					class="h-6 w-6 p-0"
				>
					<ChevronLeft class="h-3 w-3" />
				</Button>

				{#if tableInstance}
					{@const currentPage = tableInstance.getState().pagination.pageIndex}
					{@const totalPages = tableInstance.getPageCount()}

					{#if currentPage > 1}
						<Button
							variant="ghost"
							size="sm"
							onclick={() => tableInstance?.setPageIndex(0)}
							class="h-6 px-2 text-xs"
						>
							1
						</Button>
						{#if currentPage > 2}
							<span class="text-muted-foreground text-xs">...</span>
						{/if}
					{/if}

					{#if currentPage > 0}
						<Button
							variant="ghost"
							size="sm"
							onclick={() => tableInstance?.previousPage()}
							class="h-6 px-2 text-xs"
						>
							{currentPage}
						</Button>
					{/if}

					<Button variant="default" size="sm" class="h-6 px-2 text-xs">
						{currentPage + 1}
					</Button>

					{#if currentPage < totalPages - 1}
						<Button
							variant="ghost"
							size="sm"
							onclick={() => tableInstance?.nextPage()}
							class="h-6 px-2 text-xs"
						>
							{currentPage + 2}
						</Button>
					{/if}

					{#if currentPage < totalPages - 2}
						{#if currentPage < totalPages - 3}
							<span class="text-muted-foreground text-xs">...</span>
						{/if}
						<Button
							variant="ghost"
							size="sm"
							onclick={() => tableInstance?.setPageIndex(totalPages - 1)}
							class="h-6 px-2 text-xs"
						>
							{totalPages}
						</Button>
					{/if}
				{/if}

				<Button
					variant="ghost"
					size="sm"
					onclick={() => tableInstance?.nextPage()}
					disabled={!tableInstance?.getCanNextPage()}
					class="h-6 w-6 p-0"
				>
					<ChevronRight class="h-3 w-3" />
				</Button>
			</div>
		</div>
	{/if}
</div>
