<script lang="ts">
	import type { Script } from '$lib/commands.svelte';
	import { FileJson } from '@lucide/svelte';
	import { Button } from './ui/button';
	import type { SvelteSet } from 'svelte/reactivity';
	import { Menu, MenuItem } from '@tauri-apps/api/menu';

	interface Props {
		scripts: Script[];
		activeScriptId: number | null;
		unsavedChanges: SvelteSet<number>;
		onSelectScript?: (script: Script) => void;
		onCreateNewScript?: () => void;
		onDeleteScript?: (script: Script) => void;
	}

	const {
		scripts,
		activeScriptId,
		unsavedChanges,
		onSelectScript,
		onCreateNewScript,
		onDeleteScript
	}: Props = $props();

	function selectScript(script: Script) {
		onSelectScript?.(script);
	}

	function createNewScript() {
		onCreateNewScript?.();
	}

	function deleteScript(script: Script) {
		onDeleteScript?.(script);
	}
	async function showContextMenu(event: MouseEvent, script?: Script) {
		event.stopPropagation();
		event.preventDefault();

		try {
			const menu = await Menu.new();

			const newItem = await MenuItem.new({
				text: 'New Script',
				action: createNewScript
			});

			await menu.append(newItem);

			if (script) {
				const deleteItem = await MenuItem.new({
					text: 'Delete Script',
					action: () => {
						deleteScript(script);
					}
				});

				await menu.append(deleteItem);
			}

			await menu.popup();
		} catch (error) {
			console.error('[scripts] Failed to show context menu:', error);
		}
	}
</script>

<div
	class="scrollable-container h-full space-y-1 overflow-y-auto"
	oncontextmenu={(e) => showContextMenu(e)}
	role="menu"
	tabindex="-1"
>
	{#if scripts.length === 0}
		<div class="px-4 py-8 text-center">
			<div class="bg-muted/30 border-border/50 mb-3 inline-flex rounded-lg border p-3">
				<FileJson class="text-muted-foreground/50 h-6 w-6" />
			</div>
			<p class="text-muted-foreground mb-1 text-xs font-medium">No saved scripts yet</p>
			<p class="text-muted-foreground/70 text-xs">Save your SQL queries to access them later</p>
		</div>
	{:else}
		{#each scripts as script (script.id)}
			<div class="group">
				<Button
					variant="ghost"
					class="hover:bg-primary/20 h-auto w-full justify-start rounded-sm p-1 transition-all duration-200 {activeScriptId ===
					script.id
						? 'bg-primary/20'
						: 'hover:bg-background'}"
					onclick={() => selectScript(script)}
					oncontextmenu={(e) => showContextMenu(e, script)}
				>
					<div class="flex w-full items-center gap-3">
						<div class="flex-shrink-0 pl-1">
							<FileJson class="text-muted-foreground h-3 w-3" />
						</div>

						<div class="min-w-0 flex-1 text-left">
							<div class="text-foreground truncate text-sm font-medium">
								{script.name}
								{#if activeScriptId === script.id && unsavedChanges.has(script.id)}
									<span class="text-orange-500">*</span>
								{/if}
							</div>
							<div class="text-muted-foreground/80 truncate text-xs">
								Modified {new Date(script.updated_at * 1000).toLocaleDateString()}
							</div>
						</div>
					</div>
				</Button>
			</div>
		{/each}
	{/if}
</div>
