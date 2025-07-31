<script lang="ts">
	import { createSvelteTable, FlexRender, renderComponent } from '$lib/components/ui/data-table';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import {
		getCoreRowModel,
		getSortedRowModel,
		getFilteredRowModel,
		getPaginationRowModel,
		type ColumnDef,
		type SortingState,
		type ColumnFiltersState,
		type PaginationState,
		type ColumnSizingState
	} from '@tanstack/table-core';
	import {
		ChevronUp,
		ChevronDown,
		ChevronsUpDown,
		ChevronLeft,
		ChevronRight,
		Search
	} from '@lucide/svelte';

	interface Props {
		data: Record<string, any>[];
		columns: string[];
		table?: any;
		globalFilter?: string;
	}

	let { data, columns, table = $bindable(), globalFilter = $bindable('') }: Props = $props();

	// Table state
	let sorting = $state<SortingState>([]);
	let columnFilters = $state<ColumnFiltersState>([]);
	let columnSizing = $state<ColumnSizingState>({});
	let pagination = $state<PaginationState>({
		pageIndex: 0,
		pageSize: 50
	});

	let isResizing = $state(false);
	let resizePreviewX = $state(0);
	let resizingColumnId = $state<string | null>(null);
	let tableContainer: HTMLDivElement;

	// Create column definitions dynamically with resizing enabled
	const columnDefs = $derived<ColumnDef<Record<string, any>, any>[]>(
		columns.map((column) => ({
			accessorKey: column,
			header: ({ column: col }) => {
				return column;
			},
			cell: ({ getValue }) => {
				const value = getValue();

				if (value === null || value === undefined) {
					return { value: 'null', type: 'null' };
				}
				if (typeof value === 'boolean') {
					return { value: value ? 'true' : 'false', type: 'boolean' };
				}
				if (typeof value === 'number') {
					return { value: value.toLocaleString(), type: 'number' };
				}

				// String or other types
				return { value: String(value), type: 'string' };
			},
			size: 150,
			minSize: 50,
			maxSize: 800,
			enableResizing: true
		}))
	);

	const tableInstance = createSvelteTable({
		get data() {
			return data;
		},
		get columns() {
			return columnDefs;
		},
		state: {
			get sorting() {
				return sorting;
			},
			get columnFilters() {
				return columnFilters;
			},
			get globalFilter() {
				return globalFilter;
			},
			get pagination() {
				return pagination;
			},
			get columnSizing() {
				return columnSizing;
			}
		},
		onSortingChange: (updater) => {
			sorting = typeof updater === 'function' ? updater(sorting) : updater;
		},
		onColumnFiltersChange: (updater) => {
			columnFilters = typeof updater === 'function' ? updater(columnFilters) : updater;
		},
		onGlobalFilterChange: (updater) => {
			globalFilter = typeof updater === 'function' ? updater(globalFilter) : updater;
		},
		onPaginationChange: (updater) => {
			pagination = typeof updater === 'function' ? updater(pagination) : updater;
		},
		onColumnSizingChange: (updater) => {
			columnSizing = typeof updater === 'function' ? updater(columnSizing) : updater;
		},
		getCoreRowModel: getCoreRowModel(),
		getSortedRowModel: getSortedRowModel(),
		getFilteredRowModel: getFilteredRowModel(),
		getPaginationRowModel: getPaginationRowModel(),
		enableColumnResizing: true,
		columnResizeMode: 'onEnd' // Keep smooth behavior
	});

	// Expose table to parent
	table = tableInstance;

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
								{@const cellData = cell.getValue()}
								{@const columnWidth = cell.column.getSize()}
								{@const originalValue = cell.row.original[cell.column.id]}
								<td
									class="border-border/20 border-r px-2 py-1 align-top text-xs"
									style="width: {columnWidth}px"
									title={originalValue !== null && originalValue !== undefined
										? String(originalValue)
										: undefined}
								>
									{#if cellData && typeof cellData === 'object' && 'type' in cellData && cellData.type === 'null'}
										<span class="text-muted-foreground text-xs italic">NULL</span>
									{:else if cellData && typeof cellData === 'object' && 'type' in cellData && 'value' in cellData && cellData.type === 'boolean'}
										<span class="text-foreground">{cellData.value}</span>
									{:else if cellData && typeof cellData === 'object' && 'type' in cellData && 'value' in cellData && cellData.type === 'number'}
										<span class="text-foreground font-mono">{cellData.value}</span>
									{:else}
										<div class="text-foreground truncate" style="max-width: {columnWidth - 16}px">
											{cellData && typeof cellData === 'object' && 'value' in cellData
												? cellData.value
												: String(cellData || '')}
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
										<p class="text-xs text-muted-foreground/70">Try adjusting your search terms</p>
									</div>
								</div>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	</div>

	<!-- Pagination at bottom -->
	{#if tableInstance.getPageCount() > 1}
		<div class="border-border/30 bg-muted/20 flex flex-shrink-0 items-center border-t px-3 py-2">
			<!-- Compact left section -->
			<div class="text-muted-foreground flex items-center gap-2 text-xs">
				<span
					>Page {tableInstance.getState().pagination.pageIndex + 1} of {tableInstance.getPageCount()}</span
				>
				<span>•</span>
				<select
					bind:value={pagination.pageSize}
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
					onclick={() => tableInstance.previousPage()}
					disabled={!tableInstance.getCanPreviousPage()}
					class="h-6 w-6 p-0"
				>
					<ChevronLeft class="h-3 w-3" />
				</Button>

				<!-- Simplified pagination - show only current and adjacent pages -->
				{#if true}
					{@const currentPage = tableInstance.getState().pagination.pageIndex}
					{@const totalPages = tableInstance.getPageCount()}

					{#if currentPage > 1}
						<Button
							variant="ghost"
							size="sm"
							onclick={() => tableInstance.setPageIndex(0)}
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
							onclick={() => tableInstance.previousPage()}
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
							onclick={() => tableInstance.nextPage()}
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
							onclick={() => tableInstance.setPageIndex(totalPages - 1)}
							class="h-6 px-2 text-xs"
						>
							{totalPages}
						</Button>
					{/if}
				{/if}

				<Button
					variant="ghost"
					size="sm"
					onclick={() => tableInstance.nextPage()}
					disabled={!tableInstance.getCanNextPage()}
					class="h-6 w-6 p-0"
				>
					<ChevronRight class="h-3 w-3" />
				</Button>
			</div>
		</div>
	{/if}
</div>
