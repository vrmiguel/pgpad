import {
	type RowData,
	type ColumnDef,
	type SortingState,
	type ColumnFiltersState,
	type PaginationState,
	type ColumnSizingState,
	type ColumnSizingInfoState,
	type ColumnPinningState,
	type RowSelectionState,
	type VisibilityState,
	createTable,
	getCoreRowModel,
	getSortedRowModel,
	getFilteredRowModel,
	getPaginationRowModel
} from '@tanstack/table-core';

export interface TableState {
	sorting: SortingState;
	columnFilters: ColumnFiltersState;
	globalFilter: string;
	pagination: PaginationState;
	columnSizing: ColumnSizingState;
	columnSizingInfo: ColumnSizingInfoState;
	columnPinning: ColumnPinningState;
	rowSelection: RowSelectionState;
	columnVisibility: VisibilityState;
}

export interface DataTableOptions<TData extends RowData> {
	data: TData[];
	columns: ColumnDef<TData>[];
	initialState?: Partial<TableState>;
	enableSorting?: boolean;
	enableFiltering?: boolean;
	enablePagination?: boolean;
	enableColumnResizing?: boolean;
	onStateChange?: (state: TableState) => void;
}

export function createSvelteTable<TData extends RowData>(options: DataTableOptions<TData>) {
	console.log(`Running createSvelteTable, got ${options.data.length} rows`);

	const state = $state<TableState>({
		sorting: options.initialState?.sorting ?? [],
		columnFilters: options.initialState?.columnFilters ?? [],
		globalFilter: options.initialState?.globalFilter ?? '',
		pagination: options.initialState?.pagination ?? {
			pageIndex: 0,
			pageSize: 50
		},
		columnSizing: options.initialState?.columnSizing ?? {},
		columnSizingInfo: options.initialState?.columnSizingInfo ?? {
			startOffset: null,
			startSize: null,
			deltaOffset: null,
			deltaPercentage: null,
			isResizingColumn: false,
			columnSizingStart: []
		},
		columnPinning: options.initialState?.columnPinning ?? { left: [], right: [] },
		rowSelection: options.initialState?.rowSelection ?? {},
		columnVisibility: options.initialState?.columnVisibility ?? {}
	});

	const notifyStateChange = () => {
		options.onStateChange?.(state);
	};

	const table = createTable<TData>({
		data: options.data,
		columns: options.columns,
		state,
		onStateChange: () => {
			// Handled by the individual updaters
		},
		renderFallbackValue: null,
		onSortingChange: (updater) => {
			state.sorting = typeof updater === 'function' ? updater(state.sorting) : updater;
			notifyStateChange();
		},
		onColumnFiltersChange: (updater) => {
			state.columnFilters = typeof updater === 'function' ? updater(state.columnFilters) : updater;
			notifyStateChange();
		},
		onGlobalFilterChange: (updater) => {
			state.globalFilter = typeof updater === 'function' ? updater(state.globalFilter) : updater;
			notifyStateChange();
		},
		onPaginationChange: (updater) => {
			state.pagination = typeof updater === 'function' ? updater(state.pagination) : updater;
			notifyStateChange();
		},
		onColumnSizingChange: (updater) => {
			state.columnSizing = typeof updater === 'function' ? updater(state.columnSizing) : updater;
			notifyStateChange();
		},
		onColumnSizingInfoChange: (updater) => {
			state.columnSizingInfo =
				typeof updater === 'function' ? updater(state.columnSizingInfo) : updater;
			notifyStateChange();
		},
		onColumnPinningChange: (updater) => {
			state.columnPinning = typeof updater === 'function' ? updater(state.columnPinning) : updater;
			notifyStateChange();
		},
		onRowSelectionChange: (updater) => {
			state.rowSelection = typeof updater === 'function' ? updater(state.rowSelection) : updater;
			notifyStateChange();
		},
		onColumnVisibilityChange: (updater) => {
			state.columnVisibility =
				typeof updater === 'function' ? updater(state.columnVisibility) : updater;
			notifyStateChange();
		},
		getCoreRowModel: getCoreRowModel(),
		...(options.enableSorting !== false && { getSortedRowModel: getSortedRowModel() }),
		...(options.enableFiltering !== false && { getFilteredRowModel: getFilteredRowModel() }),
		...(options.enablePagination !== false && { getPaginationRowModel: getPaginationRowModel() }),
		...(options.enableColumnResizing !== false && {
			enableColumnResizing: true,
			columnResizeMode: 'onEnd' as const
		})
	});

	$effect(() => {
		table.setOptions(prev => ({
			...prev,
			data: options.data,
			columns: options.columns
		}));

		// Running this causes the table to update its internal state.
		// I don't exactly know why, but pagination would not work if this is not executed.
		table.getPageCount();
	});

	return {
		...table,
		state
	};
}

export function createQueryColumns(columnNames: string[]): ColumnDef<Record<string, any>>[] {
	return columnNames.map((column) => ({
		accessorKey: column,
		header: column,
		size: 150,
		minSize: 50,
		maxSize: 800,
		enableResizing: true,
		enableSorting: true
	}));
}
