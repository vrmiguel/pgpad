<script lang="ts">
	import type { Json, Row } from '$lib/commands.svelte';
	import { Button } from '$lib/components/ui/button';
	import { CellFormatter } from '$lib/utils/cell-formatter';

	import {
		ChevronUp,
		ChevronDown,
		ChevronsUpDown,
		ChevronLeft,
		ChevronRight,
		Search,
		MoreHorizontal
	} from '@lucide/svelte';

	interface Props {
		data: Row[];
		columns: string[];
		globalFilter?: string;
		selectedCellData?: Json | null;
		onJsonInspect?: (data: Json, position: { x: number; y: number }) => void;
	}

	let {
		data,
		columns,
		globalFilter = $bindable(''),
		selectedCellData = $bindable(null),
		onJsonInspect
	}: Props = $props();

	let tableContainer: HTMLDivElement;
	let hasFocus = $state(false);
	let processedData = $state<Row[]>([]);
	let sortCache = $state(new Map<string, Row[]>());
	let lastDataLength = $state(0);

	let selectedCell = $state<{ rowId: number; columnId: number } | null>(null);

	const tableState = $state({
		pagination: {
			pageIndex: 0,
			pageSize: 50
		},
		columnSizing: {} as Record<string, number>,
		globalFilter: globalFilter || '',
		resizing: {
			isResizing: false,
			columnIndex: -1,
			startX: 0,
			startWidth: 0
		},
		sorting: [] as Array<{ columnIndex: number; desc: boolean }>
	});

	function calculateColumnWidth(columnIndex: number, columnName: string, firstRow?: Row): number {
		const MIN_WIDTH = 60;
		const MAX_WIDTH = 400;
		// Some guesstimate at an average character width in pixels
		const CHAR_WIDTH = 7;
		const PADDING = 20;

		const headerWidth = columnName.length * CHAR_WIDTH;

		let contentWidth = headerWidth;
		if (firstRow && firstRow[columnIndex] != null) {
			const cellContent = String(firstRow[columnIndex]);
			contentWidth = Math.max(headerWidth, cellContent.length * CHAR_WIDTH);
		}

		const estimatedWidth = contentWidth + PADDING;
		return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, estimatedWidth));
	}

	const getColumnWidth = (columnIndex: number): number => {
		return tableState.columnSizing[columnIndex] || 60;
	};

	$effect(() => {
		if (globalFilter !== tableState.globalFilter) {
			tableState.globalFilter = globalFilter;
		}
	});

	$effect(() => {
		if (tableState.globalFilter !== globalFilter) {
			globalFilter = tableState.globalFilter;
		}
	});

	const getSortCacheKey = (sorting: Array<{ columnIndex: number; desc: boolean }>) => {
		if (sorting.length === 0) return 'unsorted';
		return sorting.map((s) => `${s.columnIndex}:${s.desc ? 'desc' : 'asc'}`).join(',');
	};

	$effect(() => {
		if (data.length !== lastDataLength) {
			// TODO: update with changes?
			sortCache.clear();
			lastDataLength = data.length;
		}

		if (tableState.sorting.length === 0) {
			processedData = data;
			return;
		}

		const cacheKey = getSortCacheKey(tableState.sorting);

		if (sortCache.has(cacheKey)) {
			console.log(`Sort cache hit for ${cacheKey}`);
			processedData = sortCache.get(cacheKey)!;
			return;
		}

		// Check if we're sorting a column we already sorted before, in the opposite direction
		if (tableState.sorting.length === 1) {
			const currentSort = tableState.sorting[0];
			const reverseKey = `${currentSort.columnIndex}:${currentSort.desc ? 'asc' : 'desc'}`;

			if (sortCache.has(reverseKey)) {
				console.log(
					`Fast reverse sort for column ${currentSort.columnIndex}: ${reverseKey} → ${cacheKey}`
				);
				const reversedData = [...sortCache.get(reverseKey)!].reverse();
				sortCache.set(cacheKey, reversedData);
				processedData = reversedData;
				return;
			}
		}

		let sortedData = [...data];
		sortedData.sort((a, b) => {
			for (const sort of tableState.sorting) {
				const aVal = a[sort.columnIndex];
				const bVal = b[sort.columnIndex];

				if (aVal == null && bVal == null) continue;
				if (aVal == null) return sort.desc ? 1 : -1;
				if (bVal == null) return sort.desc ? -1 : 1;

				let result = 0;
				if (typeof aVal === 'number' && typeof bVal === 'number') {
					result = aVal - bVal;
				} else {
					result = String(aVal).localeCompare(String(bVal));
				}

				if (result !== 0) {
					return sort.desc ? -result : result;
				}
			}
			return 0;
		});

		console.log(`Full sort performed for: ${cacheKey} (${data.length} rows)`);
		sortCache.set(cacheKey, sortedData);
		processedData = sortedData;
	});

	const pageCount = $derived(() => {
		const totalRows = processedData.length;
		const pageSize = tableState.pagination.pageSize;
		return Math.ceil(totalRows / pageSize);
	});

	const currentPageIndex = $derived(() => tableState.pagination.pageIndex);

	const visibleRowData = $derived.by(() => {
		const pageSize = tableState.pagination.pageSize;
		const pageIndex = tableState.pagination.pageIndex;
		const startIndex = pageIndex * pageSize;
		const endIndex = startIndex + pageSize;

		console.log(
			'Calculating visible rows - page:',
			pageIndex,
			'startIndex:',
			startIndex,
			'endIndex:',
			endIndex
		);

		return processedData.slice(startIndex, endIndex);
	});

	$effect(() => {
		void data;
		selectedCell = null;
		selectedCellData = null;

		if (data.length > 0 && columns.length > 0) {
			const newColumnSizing: Record<string, number> = {};
			const firstRow = data[0];

			columns.forEach((columnName, columnIndex) => {
				newColumnSizing[columnIndex] = calculateColumnWidth(columnIndex, columnName, firstRow);
			});

			tableState.columnSizing = newColumnSizing;
		} else {
			tableState.columnSizing = {};
		}
	});

	function startColumnResize(columnIndex: number, event: MouseEvent) {
		event.preventDefault();
		tableState.resizing.isResizing = true;
		tableState.resizing.columnIndex = columnIndex;
		tableState.resizing.startX = event.clientX;
		tableState.resizing.startWidth = getColumnWidth(columnIndex);

		document.addEventListener('mousemove', handleColumnResize);
		document.addEventListener('mouseup', stopColumnResize);
	}

	function handleColumnResize(event: MouseEvent) {
		if (!tableState.resizing.isResizing) return;

		const deltaX = event.clientX - tableState.resizing.startX;
		const newWidth = Math.max(50, tableState.resizing.startWidth + deltaX); // Min width 50px

		tableState.columnSizing[tableState.resizing.columnIndex] = newWidth;
	}

	function stopColumnResize() {
		tableState.resizing.isResizing = false;
		tableState.resizing.columnIndex = -1;

		document.removeEventListener('mousemove', handleColumnResize);
		document.removeEventListener('mouseup', stopColumnResize);
	}

	function handleSimpleCellClick(cellValue: Json, rowId: number, columnId: number) {
		// Deselect when clicking the selected cell
		if (selectedCell && selectedCell.rowId === rowId && selectedCell.columnId === columnId) {
			selectedCell = null;
			selectedCellData = null;
			return;
		}

		selectedCell = { rowId, columnId };
		const cellType = CellFormatter.getCellType(cellValue);
		if (cellType !== 'object') {
			selectedCellData = cellValue;
		}
	}

	function handleJsonInspectorOpen(cellValue: Json, event: MouseEvent) {
		event.stopPropagation();

		const WINDOW_SIZE = { width: 450, height: 400 };
		const VIEWPORT_MARGIN = 10;
		const BELOW_OFFSET = 30;

		const button = event.currentTarget as HTMLElement;
		const buttonRect = button.getBoundingClientRect();

		let x = buttonRect.left + buttonRect.width / 2 - WINDOW_SIZE.width / 2;
		let y = buttonRect.top - WINDOW_SIZE.height - VIEWPORT_MARGIN;

		// Clip to viewport
		x = Math.max(
			VIEWPORT_MARGIN,
			Math.min(x, window.innerWidth - WINDOW_SIZE.width - VIEWPORT_MARGIN)
		);

		if (y < VIEWPORT_MARGIN) {
			y = buttonRect.top + BELOW_OFFSET;
		}

		y = Math.max(
			VIEWPORT_MARGIN,
			Math.min(y, window.innerHeight - WINDOW_SIZE.height - VIEWPORT_MARGIN)
		);

		onJsonInspect?.(cellValue, { x, y });
	}

	function getCellValueForCopy(): string | null {
		if (!selectedCell) return null;

		try {
			const rowIndex = selectedCell.rowId;
			const colIndex = selectedCell.columnId;

			if (rowIndex >= 0 && rowIndex < data.length && colIndex >= 0 && colIndex < columns.length) {
				const cellValue = data[rowIndex][colIndex];
				return CellFormatter.formatCellForCopy(cellValue);
			}
			return null;
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

	const customNavigation = {
		setPageIndex: (pageIndex: number) => {
			tableState.pagination.pageIndex = Math.max(0, Math.min(pageIndex, pageCount() - 1));
		},
		nextPage: () => {
			if (tableState.pagination.pageIndex < pageCount() - 1) {
				tableState.pagination.pageIndex++;
			}
		},
		previousPage: () => {
			if (tableState.pagination.pageIndex > 0) {
				tableState.pagination.pageIndex--;
			}
		},
		canNextPage: () => tableState.pagination.pageIndex < pageCount() - 1,
		canPreviousPage: () => tableState.pagination.pageIndex > 0
	};

	const customSorting = {
		toggleSort: (columnIndex: number) => {
			const existingSort = tableState.sorting.find((s) => s.columnIndex === columnIndex);

			if (!existingSort) {
				// start with ascending
				tableState.sorting = [{ columnIndex, desc: false }];
			} else if (!existingSort.desc) {
				// change to descending
				existingSort.desc = true;
			} else {
				// remove sorting
				tableState.sorting = tableState.sorting.filter((s) => s.columnIndex !== columnIndex);
			}

			tableState.pagination.pageIndex = 0;
		},

		getSortDirection: (columnIndex: number): 'asc' | 'desc' | null => {
			const sort = tableState.sorting.find((s) => s.columnIndex === columnIndex);
			if (!sort) return null;
			return sort.desc ? 'desc' : 'asc';
		},

		isSorted: (columnIndex: number): boolean => {
			return tableState.sorting.some((s) => s.columnIndex === columnIndex);
		}
	};

	function navigateCell(direction: string) {
		if (!selectedCell) return;

		const rowIndex = selectedCell.rowId;
		const colIndex = selectedCell.columnId;
		const pageSize = tableState.pagination.pageSize;
		const pageIndex = tableState.pagination.pageIndex;
		const startRowIndex = pageIndex * pageSize;

		let newRowIndex = rowIndex;
		let newColIndex = colIndex;

		switch (direction) {
			case 'ArrowUp':
				newRowIndex = Math.max(0, rowIndex - 1);
				break;
			case 'ArrowDown':
				newRowIndex = Math.min(data.length - 1, rowIndex + 1);
				break;
			case 'ArrowLeft':
				newColIndex = Math.max(0, colIndex - 1);
				break;
			case 'ArrowRight':
				newColIndex = Math.min(columns.length - 1, colIndex + 1);
				break;
		}

		// Check if we need to change pages
		const newPageIndex = Math.floor(newRowIndex / pageSize);
		if (newPageIndex !== pageIndex) {
			tableState.pagination.pageIndex = newPageIndex;
		}

		selectedCell = { rowId: newRowIndex, columnId: newColIndex };
		if (newRowIndex < data.length && newColIndex < columns.length) {
			selectedCellData = data[newRowIndex][newColIndex];
		}

		setTimeout(() => scrollCellIntoView(newRowIndex - startRowIndex, newColIndex), 0);
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

<svelte:window on:keydown={handleKeydown} />

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
	class="relative flex h-full flex-col"
	tabindex="0"
	role="application"
	onfocus={() => (hasFocus = true)}
	onblur={() => (hasFocus = false)}
>
	<!-- Table content -->
	<div class="table-container flex-1 overflow-hidden" bind:this={tableContainer}>
		<div class="scrollable-container h-full overflow-auto">
			<table class="w-full table-fixed border-collapse text-xs">
				<thead class="bg-accent border-border sticky top-0 z-10 border-b shadow-sm">
					<tr>
						{#each columns as columnName, columnIndex (columnName)}
							{@const columnWidth = getColumnWidth(columnIndex)}
							<th
								class="text-foreground bg-muted/95 border-border/40 column-header relative border-r px-2 py-0.5 text-left align-middle text-xs font-medium"
								style="--column-width: {columnWidth}px"
							>
								<div class="flex items-center justify-between">
									<Button
										variant="ghost"
										size="sm"
										class="hover:bg-accent/30 -ml-1 h-5 flex-1 justify-start p-1 text-xs font-medium"
										onclick={() => customSorting.toggleSort(columnIndex)}
									>
										{columnName}
										{#if customSorting.getSortDirection(columnIndex) === 'asc'}
											<ChevronUp class="text-muted-foreground ml-1 h-3 w-3" />
										{:else if customSorting.getSortDirection(columnIndex) === 'desc'}
											<ChevronDown class="text-muted-foreground ml-1 h-3 w-3" />
										{:else}
											<ChevronsUpDown class="text-muted-foreground/40 ml-1 h-3 w-3" />
										{/if}
									</Button>
									<!-- Column resize handle -->
									<button
										class="absolute top-0 right-0 h-full w-1 cursor-col-resize border-none bg-transparent transition-colors select-none hover:bg-blue-400 focus:bg-blue-400 focus:outline-none active:bg-blue-500 {tableState
											.resizing.isResizing && tableState.resizing.columnIndex === columnIndex
											? 'bg-blue-500'
											: ''}"
										onmousedown={(event) => startColumnResize(columnIndex, event)}
										type="button"
										aria-label="Resize column"
									></button>
								</div>
							</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each visibleRowData as rowData, index (currentPageIndex() * tableState.pagination.pageSize + index)}
						<tr
							class="border-border/40 hover:bg-muted/50 group border-b transition-colors {index %
								2 ===
							0
								? 'bg-background'
								: 'bg-card/30'}"
						>
							{#each rowData as cellValue, colIndex (colIndex)}
								{@const globalRowIndex =
									currentPageIndex() * tableState.pagination.pageSize + index}
								{@const columnId = colIndex}
								{@const rowId = globalRowIndex}
								{@const isSelected = selectedCell
									? selectedCell.rowId === rowId && selectedCell.columnId === columnId
									: false}
								{@const cellType = CellFormatter.getCellType(cellValue)}
								{@const displayValue = CellFormatter.formatCellDisplay(cellValue)}
								{@const columnWidth = getColumnWidth(colIndex)}
								<td
									class="border-border/30 table-cell border-r px-0 py-0 align-top text-xs transition-colors {isSelected
										? 'bg-primary/10 ring-primary/40 ring-1'
										: ''}"
									style="--column-width: {columnWidth}px"
									role="gridcell"
									aria-selected={isSelected}
								>
									{#if cellType === 'object'}
										<div class="group/json-cell relative h-full w-full">
											<button
												class="hover:bg-accent/50 group-hover:bg-accent/40 focus:ring-primary/40 h-full w-full cursor-pointer border-none bg-transparent px-2 py-1 text-left transition-colors select-none focus:ring-1 focus:outline-none"
												title={CellFormatter.formatCellTitle(cellValue)}
												onclick={() => handleSimpleCellClick(cellValue, rowId, columnId)}
											>
												<div class="cell-content flex items-center pr-6">
													<span class="min-w-0 flex-1 truncate font-mono text-xs"
														>{displayValue}</span
													>
												</div>
											</button>
											<button
												class="hover:bg-accent/60 focus:ring-primary/30 absolute top-1/2 right-1 -translate-y-1/2 rounded p-0.5 transition-all duration-150 focus:ring-1 focus:outline-none"
												title="Inspect JSON"
												onclick={(e) => handleJsonInspectorOpen(cellValue, e)}
												type="button"
											>
												<MoreHorizontal
													class="text-muted-foreground/80 hover:text-foreground h-3 w-3"
												/>
											</button>
										</div>
									{:else}
										<button
											class="hover:bg-accent/50 group-hover:bg-accent/40 focus:ring-primary/40 h-full w-full cursor-pointer border-none bg-transparent px-2 py-1 text-left transition-colors select-none focus:ring-1 focus:outline-none"
											title={CellFormatter.formatCellTitle(cellValue)}
											onclick={() => handleSimpleCellClick(cellValue, rowId, columnId)}
										>
											{#if cellType === 'null'}
												<span class="cell-content text-muted-foreground text-xs italic"
													>{displayValue}</span
												>
											{:else if cellType === 'boolean'}
												<span class="cell-content text-foreground">{displayValue}</span>
											{:else if cellType === 'number'}
												<span class="cell-content text-foreground font-mono">{displayValue}</span>
											{:else}
												<div class="cell-content text-foreground">
													{displayValue}
												</div>
											{/if}
										</button>
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

	{console.log(
		'Pagination render - pageCount:',
		pageCount(),
		'dataLength:',
		data.length,
		'pageSize:',
		tableState.pagination.pageSize
	)}
	{#if pageCount() > 1}
		<div class="border-border/30 bg-muted/20 flex flex-shrink-0 items-center border-t px-3 py-2">
			<div class="text-muted-foreground flex items-center gap-2 text-xs">
				<span>Page {currentPageIndex() + 1} of {pageCount()}</span>
				<span>•</span>
				<select
					bind:value={tableState.pagination.pageSize}
					class="border-border bg-background focus:ring-ring h-6 w-12 rounded border text-xs focus:ring-1"
				>
					{#each [25, 50, 100, 200] as pageSize (pageSize)}
						<option value={pageSize}>{pageSize}</option>
					{/each}
				</select>
				<span>rows</span>
			</div>

			<div class="flex-1"></div>

			<div class="flex items-center gap-1">
				<Button
					variant="ghost"
					size="sm"
					onclick={() => customNavigation.previousPage()}
					disabled={!customNavigation.canPreviousPage()}
					class="h-6 w-6 p-0"
				>
					<ChevronLeft class="h-3 w-3" />
				</Button>

				{#if currentPageIndex() > 1}
					<Button
						variant="ghost"
						size="sm"
						onclick={() => customNavigation.setPageIndex(0)}
						class="h-6 px-2 text-xs"
					>
						1
					</Button>
					{#if currentPageIndex() > 2}
						<span class="text-muted-foreground text-xs">...</span>
					{/if}
				{/if}

				{#if currentPageIndex() > 0}
					<Button
						variant="ghost"
						size="sm"
						onclick={() => customNavigation.previousPage()}
						class="h-6 px-2 text-xs"
					>
						{currentPageIndex()}
					</Button>
				{/if}

				<Button variant="default" size="sm" class="h-6 px-2 text-xs">
					{currentPageIndex() + 1}
				</Button>

				{#if currentPageIndex() < pageCount() - 1}
					<Button
						variant="ghost"
						size="sm"
						onclick={() => customNavigation.nextPage()}
						class="h-6 px-2 text-xs"
					>
						{currentPageIndex() + 2}
					</Button>
				{/if}

				{#if currentPageIndex() < pageCount() - 2}
					{#if currentPageIndex() < pageCount() - 3}
						<span class="text-muted-foreground text-xs">...</span>
					{/if}
					<Button
						variant="ghost"
						size="sm"
						onclick={() => customNavigation.setPageIndex(pageCount() - 1)}
						class="h-6 px-2 text-xs"
					>
						{pageCount()}
					</Button>
				{/if}

				<Button
					variant="ghost"
					size="sm"
					onclick={() => customNavigation.nextPage()}
					disabled={!customNavigation.canNextPage()}
					class="h-6 w-6 p-0"
				>
					<ChevronRight class="h-3 w-3" />
				</Button>
			</div>
		</div>
	{/if}
</div>

<style>
	.table-container {
		contain: layout style paint;
		isolation: isolate;
	}

	.table-fixed {
		table-layout: fixed;
		contain: layout style;
	}

	.column-header {
		width: var(--column-width);
	}

	.table-cell {
		width: var(--column-width);
		overflow: hidden;
		text-overflow: ellipsis;
		contain: layout style;
	}

	.cell-content {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
