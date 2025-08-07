<script lang="ts">
	import { X, Plus, Circle } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import type { Script } from '$lib/commands.svelte';
	import type { SvelteSet } from 'svelte/reactivity';

	interface Props {
		openScripts: Script[];
		activeScriptId: number | null;
		unsavedChanges: SvelteSet<number>;
		onTabSelect: (scriptId: number) => void;
		onTabClose: (scriptId: number) => void;
		onNewScript: () => void;
		onScriptRename: (scriptId: number, newName: string) => void;
	}

	let {
		openScripts,
		activeScriptId,
		unsavedChanges,
		onTabSelect,
		onTabClose,
		onNewScript,
		onScriptRename
	}: Props = $props();

	let editingScriptId = $state<number | null>(null);
	let editingName = $state('');
	let nameInput = $state<HTMLInputElement>();

	function handleTabClick(scriptId: number) {
		// Don't switch tabs while editing
		if (editingScriptId === scriptId) return;

		// Clear any ongoing editing when switching tabs
		if (editingScriptId !== null) {
			editingScriptId = null;
			editingName = '';
		}

		onTabSelect(scriptId);
	}

	function handleTabClose(e: Event, scriptId: number) {
		e.stopPropagation();
		onTabClose(scriptId);
	}

	function startEditingName(scriptId: number, currentName: string) {
		editingScriptId = scriptId;
		editingName = currentName;
		// Focus the input after it renders
		setTimeout(() => {
			if (nameInput) {
				nameInput.focus();
				nameInput.select();
			}
		}, 0);
	}

	function finishEditingName() {
		if (editingScriptId === null) return;

		const trimmedName = editingName.trim();
		if (trimmedName && trimmedName !== openScripts.find((s) => s.id === editingScriptId)?.name) {
			onScriptRename(editingScriptId, trimmedName);
		}

		editingScriptId = null;
		editingName = '';
	}

	function handleNameKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			e.stopPropagation();
			finishEditingName();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			editingScriptId = null;
			editingName = '';
		}
	}

	// Safe check for unsaved changes
	function hasUnsavedChanges(scriptId: number): boolean {
		return (
			unsavedChanges && typeof unsavedChanges.has === 'function' && unsavedChanges.has(scriptId)
		);
	}
</script>

<div class="bg-card border-border/50 flex items-center overflow-hidden border-b">
	<!-- Tab bar -->
	<div class="flex flex-1 overflow-x-auto">
		{#each openScripts as script (script.id)}
			<div
				class="group border-border/30 flex max-w-48 min-w-0 items-center border-r {activeScriptId ===
				script.id
					? 'bg-background border-b-primary border-b-2'
					: ''}"
			>
				<!-- Tab content area (clickable) -->
				<button
					type="button"
					class="hover:bg-muted/30 flex min-w-0 flex-1 items-center gap-2 px-3 py-2 text-sm transition-colors"
					onclick={() => handleTabClick(script.id)}
					ondblclick={() => startEditingName(script.id, script.name)}
				>
					<!-- Script name -->
					{#if editingScriptId === script.id}
						<input
							bind:this={nameInput}
							bind:value={editingName}
							onkeydown={handleNameKeydown}
							onblur={finishEditingName}
							onclick={(e) => e.stopPropagation()}
							class="focus:ring-primary h-6 border-none bg-transparent p-0 text-sm font-medium shadow-none focus:ring-1 focus:outline-none"
						/>
					{:else}
						<span class="truncate font-medium">
							{script.name}
						</span>
					{/if}

					<!-- Unsaved changes indicator -->
					{#if hasUnsavedChanges(script.id)}
						<Circle class="h-2 w-2 flex-shrink-0 fill-orange-500 text-orange-500" />
					{/if}
				</button>

				<!-- Close button (separate from tab button) -->
				<button
					type="button"
					class="hover:bg-destructive/10 hover:text-destructive mr-2 flex-shrink-0 rounded p-0.5 opacity-0 transition-all group-hover:opacity-100"
					onclick={(e) => handleTabClose(e, script.id)}
					title="Close tab"
				>
					<X class="h-3 w-3" />
				</button>
			</div>
		{/each}
	</div>

	<!-- New tab button -->
	<Button
		variant="ghost"
		size="sm"
		class="border-border/30 hover:bg-muted/30 rounded-none border-l"
		onclick={onNewScript}
		title="New Script"
	>
		<Plus class="h-4 w-4" />
	</Button>
</div>
