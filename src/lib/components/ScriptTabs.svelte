<script lang="ts">
	import { X, Plus, Circle } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import type { Script } from '$lib/commands.svelte';

	interface Props {
		openScripts: Script[];
		activeScriptId: number | null;
		unsavedChanges: Set<number>;
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
		if (trimmedName && trimmedName !== openScripts.find(s => s.id === editingScriptId)?.name) {
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
		return unsavedChanges && typeof unsavedChanges.has === 'function' && unsavedChanges.has(scriptId);
	}
</script>

<div class="flex items-center bg-white border-b border-border/50 overflow-hidden">
	<!-- Tab bar -->
	<div class="flex flex-1 overflow-x-auto">
		{#each openScripts as script (script.id)}
			<div class="group flex items-center border-r border-border/30 min-w-0 max-w-48 {activeScriptId === script.id ? 'bg-background border-b-2 border-b-primary' : ''}">
				<!-- Tab content area (clickable) -->
				<button
					type="button"
					class="flex items-center gap-2 px-3 py-2 text-sm hover:bg-muted/30 transition-colors flex-1 min-w-0"
					onclick={() => handleTabClick(script.id)}
					ondblclick={() => startEditingName(script.id, script.name)}
				>
					<!-- Script name -->
					{#if editingScriptId === script.id}
						<Input
							bind:this={nameInput}
							bind:value={editingName}
							onkeydown={handleNameKeydown}
							onblur={finishEditingName}
							onclick={(e) => e.stopPropagation()}
							class="h-6 text-sm font-medium bg-transparent border-none shadow-none p-0 focus:ring-1 focus:ring-primary"
						/>
					{:else}
						<span class="truncate font-medium">
							{script.name}
						</span>
					{/if}
					
					<!-- Unsaved changes indicator -->
					{#if hasUnsavedChanges(script.id)}
						<Circle class="w-2 h-2 fill-orange-500 text-orange-500 flex-shrink-0" />
					{/if}
				</button>
				
				<!-- Close button (separate from tab button) -->
				<button
					type="button"
					class="opacity-0 group-hover:opacity-100 hover:bg-red-100 hover:text-red-600 rounded p-0.5 transition-all flex-shrink-0 mr-2"
					onclick={(e) => handleTabClose(e, script.id)}
					title="Close tab"
				>
					<X class="w-3 h-3" />
				</button>
			</div>
		{/each}
	</div>
	
	<!-- New tab button -->
	<Button
		variant="ghost"
		size="sm"
		class="border-l border-border/30 rounded-none hover:bg-muted/30"
		onclick={onNewScript}
		title="New Script"
	>
		<Plus class="w-4 h-4" />
	</Button>
</div> 