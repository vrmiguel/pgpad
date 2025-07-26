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
		type PaginationState
	} from '@tanstack/table-core';
	import { ChevronUp, ChevronDown, ChevronsUpDown, ChevronLeft, ChevronRight, Search } from '@lucide/svelte';

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
	let pagination = $state<PaginationState>({
		pageIndex: 0,
		pageSize: 50
	});

	// Create column definitions dynamically
	const columnDefs = $derived<ColumnDef<Record<string, any>, any>[]>(
		columns.map((column) => ({
			accessorKey: column,
			header: ({ column: col }) => {
				// Return a simple string, we'll handle sorting in the template
				return column;
			},
			cell: ({ getValue }) => {
				const value = getValue();
				// Handle different data types with simple string returns
				if (value === null || value === undefined) {
					return 'NULL';
				}
				if (typeof value === 'boolean') {
					return value ? 'true' : 'false';
				}
				if (typeof value === 'number') {
					return value.toLocaleString();
				}
				// String or other types
				const stringValue = String(value);
				return stringValue.length > 100 ? stringValue.substring(0, 100) + '...' : stringValue;
			}
		}))
	);

	// Create the table
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
		getCoreRowModel: getCoreRowModel(),
		getSortedRowModel: getSortedRowModel(),
		getFilteredRowModel: getFilteredRowModel(),
		getPaginationRowModel: getPaginationRowModel()
	});

	// Expose table to parent
	table = tableInstance;
</script>

<div class="flex flex-col h-full">
	<!-- Table content - constrain height to force scrolling -->
	<div class="flex-1 min-h-0">
		<div class="h-full overflow-auto">
			<table class="w-full text-sm">
				<thead class="bg-muted/80 backdrop-blur-sm sticky top-0 z-10 border-b border-border">
					{#each tableInstance.getHeaderGroups() as headerGroup}
						<tr>
							{#each headerGroup.headers as header}
								<th class="h-11 px-4 text-left align-middle font-semibold text-foreground bg-muted/90 backdrop-blur-sm">
									{#if !header.isPlaceholder}
										<Button
											variant="ghost"
											size="sm"
											class="h-8 p-0 font-semibold hover:bg-accent/50 -ml-2"
											onclick={() => header.column.toggleSorting(header.column.getIsSorted() === 'asc')}
										>
											<FlexRender
												content={header.column.columnDef.header}
												context={header.getContext()}
											/>
											{#if header.column.getIsSorted() === 'asc'}
												<ChevronUp class="ml-1 h-3 w-3 text-muted-foreground" />
											{:else if header.column.getIsSorted() === 'desc'}
												<ChevronDown class="ml-1 h-3 w-3 text-muted-foreground" />
											{:else}
												<ChevronsUpDown class="ml-1 h-3 w-3 text-muted-foreground/60" />
											{/if}
										</Button>
									{/if}
								</th>
							{/each}
						</tr>
					{/each}
				</thead>
				<tbody>
					{#each tableInstance.getRowModel().rows as row}
						<tr class="border-b border-border/50 hover:bg-muted/30 transition-colors">
							{#each row.getVisibleCells() as cell}
								{@const value = cell.getValue()}
								<td class="px-4 py-3 align-middle">
									{#if value === null || value === undefined}
										<span class="text-muted-foreground italic text-xs bg-muted/30 px-1.5 py-0.5 rounded">NULL</span>
									{:else if typeof value === 'boolean'}
										<span class="inline-flex items-center px-2 py-1 rounded-full text-xs font-medium {value ? 'bg-success-light/30 text-success-foreground border border-success/20' : 'bg-muted/40 text-muted-foreground border border-border'}">
											{value ? 'true' : 'false'}
										</span>
									{:else if typeof value === 'number'}
										<span class="font-mono text-foreground">{value.toLocaleString()}</span>
									{:else}
										{@const stringValue = String(value)}
										{#if stringValue.length > 100}
											<span class="truncate max-w-xs block text-foreground" title={stringValue}>
												{stringValue}
											</span>
										{:else}
											<span class="text-foreground">{stringValue}</span>
										{/if}
									{/if}
								</td>
							{/each}
						</tr>
					{:else}
						<tr>
							<td colspan={columns.length} class="h-32 text-center text-muted-foreground">
								<div class="flex flex-col items-center gap-2">
									<div class="w-12 h-12 rounded-full bg-muted/20 flex items-center justify-center">
										<Search class="w-5 h-5 text-muted-foreground/50" />
									</div>
									<div>
										<p class="text-sm font-medium">No results found</p>
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
		<div class="flex items-center justify-between pt-3 px-4 flex-shrink-0 border-t border-border/30">
			<div class="flex items-center gap-3">
				<p class="text-sm text-muted-foreground">
					Page {tableInstance.getState().pagination.pageIndex + 1} of {tableInstance.getPageCount()}
				</p>
				<div class="flex items-center gap-2">
					<select
						bind:value={pagination.pageSize}
						class="h-8 w-16 rounded-md border border-border bg-background text-sm focus:ring-2 focus:ring-ring focus:ring-offset-1"
					>
						{#each [25, 50, 100, 200] as pageSize}
							<option value={pageSize}>{pageSize}</option>
						{/each}
					</select>
					<span class="text-sm text-muted-foreground">rows</span>
				</div>
			</div>

			<div class="flex items-center gap-1">
				<Button
					variant="outline"
					size="sm"
					onclick={() => tableInstance.previousPage()}
					disabled={!tableInstance.getCanPreviousPage()}
					class="h-8 w-8 p-0"
				>
					<ChevronLeft class="h-4 w-4" />
				</Button>
				
				<!-- Page numbers -->
				{#each Array.from({ length: Math.min(5, tableInstance.getPageCount()) }, (_, i) => {
					const currentPage = tableInstance.getState().pagination.pageIndex;
					const totalPages = tableInstance.getPageCount();
					let startPage = Math.max(0, currentPage - 2);
					let endPage = Math.min(totalPages - 1, startPage + 4);
					startPage = Math.max(0, endPage - 4);
					return startPage + i;
				}) as pageIndex}
					<Button
						variant={pageIndex === tableInstance.getState().pagination.pageIndex ? "default" : "outline"}
						size="sm"
						onclick={() => tableInstance.setPageIndex(pageIndex)}
						class="h-8 w-8 p-0"
					>
						{pageIndex + 1}
					</Button>
				{/each}

				<Button
					variant="outline"
					size="sm"
					onclick={() => tableInstance.nextPage()}
					disabled={!tableInstance.getCanNextPage()}
					class="h-8 w-8 p-0"
				>
					<ChevronRight class="h-4 w-4" />
				</Button>
			</div>
		</div>
	{/if}
</div> 