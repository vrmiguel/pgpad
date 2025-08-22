<script lang="ts">
	import TabBar from '$lib/components/ui/TabBar.svelte';
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

	// Safe check for unsaved changes
	function hasUnsavedChanges(scriptId: number): boolean {
		return (
			unsavedChanges && typeof unsavedChanges.has === 'function' && unsavedChanges.has(scriptId)
		);
	}

	function getScriptStatus(script: any): 'normal' | 'modified' | 'error' {
		return hasUnsavedChanges(script.id) ? 'modified' : 'normal';
	}

	// Type-safe wrapper functions
	function handleTabSelect(tabId: string | number) {
		onTabSelect(tabId as number);
	}

	function handleTabClose(tabId: string | number) {
		onTabClose(tabId as number);
	}

	function handleTabRename(tabId: string | number, newName: string) {
		onScriptRename(tabId as number, newName);
	}
</script>

<TabBar
	tabs={openScripts}
	activeTabId={activeScriptId}
	onTabSelect={handleTabSelect}
	onTabClose={handleTabClose}
	onNewTab={onNewScript}
	onTabRename={handleTabRename}
	showCloseButton={true}
	showNewTabButton={true}
	allowRename={true}
	getTabStatus={getScriptStatus}
	newTabLabel="New Script"
	closeTabLabel="Close tab"
/>
