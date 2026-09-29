<script lang="ts">
	import TabBar from '$lib/components/ui/TabBar.svelte';
	import { tabs } from '$lib/stores/tabs.svelte';
	import { backend } from '$lib/backend';
	import { onDestroy, onMount } from 'svelte';

	const allTabs = $derived(
		tabs.all.map((tab) => ({
			id: tab.id,
			name: tab.type === 'table-view' ? `📋 ${tab.title}` : tab.title,
			isDirty: tab.isDirty,
			canRename: tab.canRename
		}))
	);

	function handleTabSelect(tabId: string) {
		tabs.switchToTab(tabId);
	}

	function handleTabClose(tabId: string) {
		tabs.closeTab(tabId);
	}

	function handleNewScript() {
		tabs.createNewScript();
	}

	function handleScriptRename(tabId: string, newName: string) {
		tabs.renameScript(tabId, newName);
	}

	function getScriptStatus(tab: { isDirty: boolean }): 'normal' | 'modified' | 'error' {
		return tab.isDirty ? 'modified' : 'normal';
	}

	let unlistenNewTab: (() => void) | null = null;
	let unlistenCloseTab: (() => void) | null = null;
	onMount(async () => {
		unlistenNewTab = await backend.listen('new_tab', handleNewScript);
		unlistenCloseTab = await backend.listen('close_tab', () => {
			const activeId = tabs.activeId;
			if (activeId !== null) {
				handleTabClose(activeId);
			}
		});
	});
	onDestroy(() => {
		unlistenNewTab?.();
		unlistenCloseTab?.();
	});
</script>

<TabBar
	tabs={allTabs}
	activeTabId={tabs.activeId}
	onTabSelect={handleTabSelect}
	onTabClose={handleTabClose}
	onNewTab={handleNewScript}
	onTabRename={handleScriptRename}
	showCloseButton={true}
	showNewTabButton={true}
	allowRename={true}
	getTabStatus={getScriptStatus}
	newTabLabel="New Script"
	closeTabLabel="Close tab"
/>
