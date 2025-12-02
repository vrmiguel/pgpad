<script lang="ts">
  import { Commands } from '$lib/commands.svelte';

  interface Props {
    selectedConnection: string | null;
  }

  let { selectedConnection }: Props = $props();
  let loading = $state(false);
  let page = $state(0);
  let pageSize = $state(50);
  let totalPages = $state(0);
  let data: Array<{
    schema_name: string;
    table_name: string;
    index_name: string;
    is_unique: boolean;
    is_primary: boolean;
  }> = $state([]);

  async function load() {
    if (!selectedConnection) return;
    loading = true;
    try {
      const raw = await Commands.getMssqlIndexes(selectedConnection, page, pageSize);
      const parsed = JSON.parse(raw || '{}');
      data = parsed.data || [];
      totalPages = parsed.total_pages || 0;
    } catch {
      data = [];
      totalPages = 0;
    } finally {
      loading = false;
    }
  }

  function nextPage() {
    if (page + 1 < totalPages) {
      page += 1;
      load();
    }
  }

  function prevPage() {
    if (page > 0) {
      page -= 1;
      load();
    }
  }

  $effect(() => {
    const c = selectedConnection;
    if (c) {
      load();
    } else {
      data = [];
      totalPages = 0;
    }
  });
</script>

<div class="flex h-full flex-col gap-3">
  <div class="flex items-center gap-2 p-2">
    <span class="text-xs text-muted-foreground">Indexes</span>
    <div class="ml-auto text-xs text-muted-foreground">Page {page + 1} / {Math.max(1, totalPages)}</div>
  </div>

  <div class="min-h-0 flex-1 overflow-auto">
    {#if loading}
      <div class="p-4 text-sm">Loading...</div>
    {:else}
      <table class="w-full text-sm">
        <thead>
          <tr class="text-left">
            <th class="p-2">Schema</th>
            <th class="p-2">Table</th>
            <th class="p-2">Index</th>
            <th class="p-2">Unique</th>
            <th class="p-2">Primary</th>
          </tr>
        </thead>
        <tbody>
          {#each data as it (it.schema_name + '.' + it.table_name + '.' + it.index_name)}
            <tr class="border-t">
              <td class="p-2">{it.schema_name}</td>
              <td class="p-2">{it.table_name}</td>
              <td class="p-2">{it.index_name}</td>
              <td class="p-2">{it.is_unique ? 'Yes' : 'No'}</td>
              <td class="p-2">{it.is_primary ? 'Yes' : 'No'}</td>
            </tr>
          {/each}
          {#if data.length === 0}
            <tr>
              <td class="p-4 text-muted-foreground" colspan="5">No indexes</td>
            </tr>
          {/if}
        </tbody>
      </table>
    {/if}
  </div>

  <div class="flex items-center gap-2 p-2">
    <button class="border rounded-sm px-3 py-1 text-sm enabled:hover:bg-accent disabled:opacity-50" onclick={prevPage} disabled={loading || page <= 0}>Prev</button>
    <span class="text-xs">Page {page + 1}</span>
    <button class="border rounded-sm px-3 py-1 text-sm enabled:hover:bg-accent disabled:opacity-50" onclick={nextPage} disabled={loading || page + 1 >= totalPages}>Next</button>
    <div class="ml-auto flex items-center gap-2">
      <label class="text-xs" for="mssql_page_size">Page Size</label>
      <select id="mssql_page_size" class="border rounded-sm bg-background px-2 py-1 text-sm" bind:value={pageSize} onchange={() => { page = 0; load(); }}>
        <option value={25}>25</option>
        <option value={50}>50</option>
        <option value={100}>100</option>
      </select>
    </div>
  </div>
</div>
