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
	}

	let { data, columns }: Props = $props();

	// Table state
	let sorting = $state<SortingState>([]);
	let columnFilters = $state<ColumnFiltersState>([]);
	let globalFilter = $state('');
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
	const table = createSvelteTable({
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
</script>

<div class="space-y-4">
	<!-- Search and Controls -->
	<div class="flex items-center justify-between gap-4">
		<div class="flex items-center gap-2 flex-1 max-w-sm">
			<Search class="w-4 h-4 text-muted-foreground" />
			<Input
				placeholder="Search all columns..."
				bind:value={globalFilter}
				class="h-8"
			/>
		</div>
		
		<div class="flex items-center gap-2 text-sm text-muted-foreground">
			<span>
				{table.getFilteredRowModel().rows.length} of {table.getCoreRowModel().rows.length} row(s)
			</span>
		</div>
	</div>

	<!-- Table -->
	<div class="rounded-lg border border-border shadow-sm overflow-hidden">
		<div class="overflow-auto max-h-[400px]">
			<table class="w-full text-sm">
				<thead class="bg-muted/50 sticky top-0">
					{#each table.getHeaderGroups() as headerGroup}
						<tr class="border-b border-border">
							{#each headerGroup.headers as header}
								<th class="h-12 px-4 text-left align-middle font-semibold text-muted-foreground">
									{#if !header.isPlaceholder}
										<Button
											variant="ghost"
											size="sm"
											class="h-8 p-0 font-semibold hover:bg-accent"
											onclick={() => header.column.toggleSorting(header.column.getIsSorted() === 'asc')}
										>
											<FlexRender
												content={header.column.columnDef.header}
												context={header.getContext()}
											/>
											{#if header.column.getIsSorted() === 'asc'}
												<ChevronUp class="ml-2 h-4 w-4" />
											{:else if header.column.getIsSorted() === 'desc'}
												<ChevronDown class="ml-2 h-4 w-4" />
											{:else}
												<ChevronsUpDown class="ml-2 h-4 w-4" />
											{/if}
										</Button>
									{/if}
								</th>
							{/each}
						</tr>
					{/each}
				</thead>
				<tbody>
					{#each table.getRowModel().rows as row}
						<tr class="border-b border-border hover:bg-muted/50 transition-colors">
							{#each row.getVisibleCells() as cell}
								{@const value = cell.getValue()}
								<td class="p-4 align-middle">
									{#if value === null || value === undefined}
										<span class="text-muted-foreground italic text-xs">NULL</span>
									{:else if typeof value === 'boolean'}
										<span class="inline-flex items-center px-2 py-1 rounded-full text-xs font-medium {value ? 'bg-success-light/20 text-success-foreground' : 'bg-muted/20 text-muted-foreground'}">
											{value ? 'true' : 'false'}
										</span>
									{:else if typeof value === 'number'}
										<span class="font-mono">{value.toLocaleString()}</span>
									{:else}
										{@const stringValue = String(value)}
										{#if stringValue.length > 100}
											<span class="truncate max-w-xs block" title={stringValue}>
												{stringValue}
											</span>
										{:else}
											{stringValue}
										{/if}
									{/if}
								</td>
							{/each}
						</tr>
					{:else}
						<tr>
							<td colspan={columns.length} class="h-24 text-center text-muted-foreground">
								No results found.
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	</div>

	<!-- Pagination -->
	{#if table.getPageCount() > 1}
		<div class="flex items-center justify-between">
			<div class="flex items-center gap-2">
				<p class="text-sm text-muted-foreground">
					Page {table.getState().pagination.pageIndex + 1} of {table.getPageCount()}
				</p>
				<select
					bind:value={pagination.pageSize}
					class="h-8 w-16 rounded border border-border bg-background text-sm"
				>
					{#each [25, 50, 100, 200] as pageSize}
						<option value={pageSize}>{pageSize}</option>
					{/each}
				</select>
				<span class="text-sm text-muted-foreground">rows per page</span>
			</div>

			<div class="flex items-center gap-2">
				<Button
					variant="outline"
					size="sm"
					onclick={() => table.previousPage()}
					disabled={!table.getCanPreviousPage()}
					class="h-8 w-8 p-0"
				>
					<ChevronLeft class="h-4 w-4" />
				</Button>
				
				<!-- Page numbers -->
				{#each Array.from({ length: Math.min(5, table.getPageCount()) }, (_, i) => {
					const currentPage = table.getState().pagination.pageIndex;
					const totalPages = table.getPageCount();
					let startPage = Math.max(0, currentPage - 2);
					let endPage = Math.min(totalPages - 1, startPage + 4);
					startPage = Math.max(0, endPage - 4);
					return startPage + i;
				}) as pageIndex}
					<Button
						variant={pageIndex === table.getState().pagination.pageIndex ? "default" : "outline"}
						size="sm"
						onclick={() => table.setPageIndex(pageIndex)}
						class="h-8 w-8 p-0"
					>
						{pageIndex + 1}
					</Button>
				{/each}

				<Button
					variant="outline"
					size="sm"
					onclick={() => table.nextPage()}
					disabled={!table.getCanNextPage()}
					class="h-8 w-8 p-0"
				>
					<ChevronRight class="h-4 w-4" />
				</Button>
			</div>
		</div>
	{/if}
</div> 