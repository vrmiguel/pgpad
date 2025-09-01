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

	function hasUnsavedChanges(scriptId: number): boolean {
		return (
			unsavedChanges && typeof unsavedChanges.has === 'function' && unsavedChanges.has(scriptId)
		);
	}

	function getScriptStatus(tab: Script): 'normal' | 'modified' | 'error' {
		return hasUnsavedChanges(tab.id) ? 'modified' : 'normal';
	}
</script>

<TabBar
	tabs={openScripts}
	activeTabId={activeScriptId}
	{onTabSelect}
	{onTabClose}
	onNewTab={onNewScript}
	onTabRename={onScriptRename}
	showCloseButton={true}
	showNewTabButton={true}
	allowRename={true}
	getTabStatus={getScriptStatus}
	newTabLabel="New Script"
	closeTabLabel="Close tab"
/>
