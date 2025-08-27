<script lang="ts">
	import { FlexRender, createSvelteTable, createQueryColumns } from '$lib/components/ui/data-table';
	import type { PgRow } from '$lib/commands.svelte';
	import type { Cell } from '@tanstack/table-core';
	import { Button } from '$lib/components/ui/button';
	import { CellFormatter } from '$lib/utils/cell-formatter';

	import {
		ChevronUp,
		ChevronDown,
		ChevronsUpDown,
		ChevronLeft,
		ChevronRight,
		Search
	} from '@lucide/svelte';

	interface Props {
		data: PgRow[];
		columns: string[];
		globalFilter?: string;
		selectedCellData?: any | null;
	}

	let {
		data,
		columns,
		globalFilter = $bindable(''),
		selectedCellData = $bindable(null)
	}: Props = $props();

	let resizePreviewX = $state(0);
	let tableContainer: HTMLDivElement;
	let hasFocus = $state(false);

	const COLUMN_RESIZE = {
		MIN_WIDTH: 150,
		CHAR_WIDTH: 8,
		PADDING: 32,
		MAX_WIDTH: 600
	} as const;

	let selectedCell = $state<{ rowId: string; columnId: string } | null>(null);
	let originalColumnWidths = $state<Map<string, number>>(new Map());

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

	const table = createSvelteTable(options);

	$effect(() => {
		if (table && table.state.globalFilter !== globalFilter) {
			table.state.globalFilter = globalFilter;
		}
	});

	$effect(() => {
		if (table && globalFilter !== table.state.globalFilter) {
			globalFilter = table.state.globalFilter;
		}
	});

	$effect(() => {
		// Reset selection and column widths when data changes
		data;
		selectedCell = null;
		selectedCellData = null;
		originalColumnWidths.clear();
	});

	function handleWindowMouseMove(event: MouseEvent) {
		if (!tableContainer || !table.getState().columnSizingInfo.isResizingColumn) return;

		const containerRect = tableContainer.getBoundingClientRect();
		resizePreviewX = event.clientX - containerRect.left + tableContainer.scrollLeft;
	}

	function handleCellClick(cell: Cell<PgRow, unknown>, rowId: string, _: MouseEvent) {
		const cellValue = cell.getValue();
		const columnId = cell.column.id;

		// Deselect when clicking the selected cell
		if (selectedCell && selectedCell.rowId === rowId && selectedCell.columnId === columnId) {
			selectedCell = null;
			selectedCellData = null;
			restoreOriginalColumnWidths();
			return;
		}

		if (table && !originalColumnWidths.has(columnId)) {
			originalColumnWidths.set(columnId, cell.column.getSize());
		}

		selectedCell = { rowId, columnId };

		selectedCellData = cellValue;

		if (table && cellValue !== null && cellValue !== undefined) {
			const contentLength = String(cellValue).length;
			const minWidth = Math.max(
				COLUMN_RESIZE.MIN_WIDTH,
				Math.min(
					contentLength * COLUMN_RESIZE.CHAR_WIDTH + COLUMN_RESIZE.PADDING,
					COLUMN_RESIZE.MAX_WIDTH
				)
			);
			table.setColumnSizing((prev) => ({
				...prev,
				[columnId]: minWidth
			}));
		}
	}

	function restoreOriginalColumnWidths() {
		if (!table || originalColumnWidths.size === 0) return;

		const sizingUpdates: Record<string, number> = {};
		originalColumnWidths.forEach((originalWidth, columnId) => {
			sizingUpdates[columnId] = originalWidth;
		});

		table.setColumnSizing((prev) => ({
			...prev,
			...sizingUpdates
		}));

		originalColumnWidths.clear();
	}

	function getCellValueForCopy(): string | null {
		if (!selectedCell) return null;

		try {
			const row = table.getRow(selectedCell.rowId);
			if (!row) return null;

			const cellValue = row.getValue(selectedCell.columnId);
			return CellFormatter.formatCellForCopy(cellValue);
		} catch (error) {
			console.warn('Failed to get cell value:', error);
			return null;
		}
	}

	async function copyCellValue(textToCopy: string): Promise<void> {
		try {
			await navigator.clipboard.writeText(textToCopy);
		} catch (err) {
			console.error('Failed to copy text: ', err);
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (!hasFocus) return;

		if (!selectedCell) return;

		if (event.key === 'Escape') {
			selectedCell = null;
			selectedCellData = null;
			restoreOriginalColumnWidths();
			return;
		}

		if ((event.ctrlKey || event.metaKey) && event.key === 'c') {
			event.preventDefault();
			const textToCopy = getCellValueForCopy();
			if (textToCopy !== null) {
				copyCellValue(textToCopy);
			}
			return;
		}

		if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(event.key)) {
			event.preventDefault();
			navigateCell(event.key);
		}
	}

	function navigateCell(direction: string) {
		if (!selectedCell || !table) return;

		const row = table.getRow(selectedCell.rowId);
		const column = table.getColumn(selectedCell.columnId);

		if (row === undefined || column === undefined) return;

		const rows = table.getPaginationRowModel().rows;
		const columns = table.getVisibleLeafColumns();

		const currentRowIndex = row.index;
		const currentColumnIndex = column.getIndex();

		if (currentRowIndex === -1 || currentColumnIndex === -1) return;

		let newRowIndex = currentRowIndex;
		let newColumnIndex = currentColumnIndex;

		switch (direction) {
			case 'ArrowUp':
				newRowIndex = Math.max(0, currentRowIndex - 1);
				break;
			case 'ArrowDown':
				newRowIndex = Math.min(rows.length - 1, currentRowIndex + 1);
				break;
			case 'ArrowLeft':
				newColumnIndex = Math.max(0, currentColumnIndex - 1);
				break;
			case 'ArrowRight':
				newColumnIndex = Math.min(columns.length - 1, currentColumnIndex + 1);
				break;
		}

		if (newRowIndex !== currentRowIndex || newColumnIndex !== currentColumnIndex) {
			const row = rows[newRowIndex];
			const column = columns[newColumnIndex];
			const cell = row.getVisibleCells().find((c) => c.column.id === column.id);

			if (cell) {
				const mockEvent = new MouseEvent('click');
				handleCellClick(cell, row.id, mockEvent);
				scrollCellIntoView(newRowIndex, newColumnIndex);
			}
		}
	}

	function scrollCellIntoView(rowIndex: number, columnIndex: number) {
		const tbody = tableContainer?.querySelector('tbody');
		if (!tbody) return;

		const targetRow = tbody.children[rowIndex] as HTMLElement;
		const targetCell = targetRow?.children[columnIndex] as HTMLElement;

		if (targetCell) {
			targetCell.scrollIntoView({
				behavior: 'smooth',
				block: 'nearest',
				inline: 'nearest'
			});
		}
	}
</script>

<svelte:window on:keydown={handleKeydown} onmousemove={handleWindowMouseMove} />

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
	class="relative flex h-full flex-col"
	tabindex="0"
	role="application"
	onfocus={() => (hasFocus = true)}
	onblur={() => (hasFocus = false)}
>
	<!-- Resize preview line -->
	{#if table.getState().columnSizingInfo.isResizingColumn}
		<div
			class="pointer-events-none absolute top-0 bottom-0 z-20 w-0.5 bg-blue-500 opacity-75"
			style="left: {resizePreviewX}px"
		></div>
	{/if}

	<!-- Table content -->
	<div class="flex-1 overflow-hidden" bind:this={tableContainer}>
		<div class="h-full overflow-auto">
			<table class="w-full border-collapse text-xs" style="width: {table.getCenterTotalSize()}px">
				<thead class="bg-accent border-border sticky top-0 z-10 border-b shadow-sm">
					{#each table.getHeaderGroups() as headerGroup}
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
											{#if header.column.getCanResize()}
												<button
													class="absolute top-0 right-0 h-full w-1 cursor-col-resize border-none bg-transparent transition-colors select-none hover:bg-blue-400 focus:bg-blue-400 focus:outline-none active:bg-blue-500 {table.getState()
														.columnSizingInfo.isResizingColumn === header.column.id
														? 'bg-blue-500'
														: ''}"
													onmousedown={header.getResizeHandler()}
													ontouchstart={header.getResizeHandler()}
													type="button"
													aria-label="Resize column"
												></button>
											{/if}
										</div>
									{/if}
								</th>
							{/each}
						</tr>
					{/each}
				</thead>
				<tbody>
					{#each table.getPaginationRowModel().rows as row, index (row.id)}
						<tr
							class="border-border/40 hover:bg-muted/50 group border-b transition-colors {index %
								2 ===
							0
								? 'bg-background'
								: 'bg-card/30'}"
						>
							{#each row.getVisibleCells() as cell (cell.column.id)}
								{@const cellValue = cell.getValue()}
								{@const columnWidth = cell.column.getSize()}
								{@const isSelected = selectedCell
									? selectedCell.rowId === row.id && selectedCell.columnId === cell.column.id
									: false}
								{@const cellType = CellFormatter.getCellType(cellValue)}
								{@const displayValue = CellFormatter.formatCellDisplay(cellValue)}
								<td
									class="border-border/30 border-r px-0 py-0 align-top text-xs transition-colors {isSelected
										? 'bg-primary/10 ring-primary/40 ring-1'
										: ''}"
									style="width: {columnWidth}px"
									role="gridcell"
									aria-selected={isSelected}
								>
									<button
										class="hover:bg-accent/50 group-hover:bg-accent/40 focus:ring-primary/40 h-full w-full cursor-pointer border-none bg-transparent px-2 py-1 text-left transition-colors select-none focus:ring-1 focus:outline-none"
										title={CellFormatter.formatCellTitle(cellValue)}
										onclick={(event) => handleCellClick(cell, row.id, event)}
									>
										{#if cellType === 'null'}
											<span class="cell-content text-muted-foreground text-xs italic"
												>{displayValue}</span
											>
										{:else if cellType === 'boolean'}
											<span class="cell-content text-foreground">{displayValue}</span>
										{:else if cellType === 'number'}
											<span class="cell-content text-foreground font-mono">{displayValue}</span>
										{:else if cellType === 'object'}
											<div class="cell-content cursor-pointer">
												<span class="font-mono text-xs">{displayValue}</span>
											</div>
										{:else}
											<div
												class="cell-content text-foreground {isSelected ? '' : 'truncate'}"
												style="max-width: {isSelected ? 'none' : columnWidth - 16 + 'px'}"
											>
												{displayValue}
											</div>
										{/if}
									</button>
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
	{#if table.getPageCount() > 1}
		{@const currentPage = table.getState().pagination.pageIndex}
		{@const totalPages = table.getPageCount()}
		<div class="border-border/30 bg-muted/20 flex flex-shrink-0 items-center border-t px-3 py-2">
			<!-- Compact left section -->
			<div class="text-muted-foreground flex items-center gap-2 text-xs">
				<span>Page {table.getState().pagination.pageIndex + 1} of {table.getPageCount()}</span>
				<span>•</span>
				<select
					bind:value={table.state.pagination.pageSize}
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
					onclick={() => table.previousPage()}
					disabled={!table.getCanPreviousPage()}
					class="h-6 w-6 p-0"
				>
					<ChevronLeft class="h-3 w-3" />
				</Button>

				{#if currentPage > 1}
					<Button
						variant="ghost"
						size="sm"
						onclick={() => table.setPageIndex(0)}
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
						onclick={() => table.previousPage()}
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
						onclick={() => table.nextPage()}
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
						onclick={() => table.setPageIndex(totalPages - 1)}
						class="h-6 px-2 text-xs"
					>
						{totalPages}
					</Button>
				{/if}

				<Button
					variant="ghost"
					size="sm"
					onclick={() => table.nextPage()}
					disabled={!table.getCanNextPage()}
					class="h-6 w-6 p-0"
				>
					<ChevronRight class="h-3 w-3" />
				</Button>
			</div>
		</div>
	{/if}
</div>
