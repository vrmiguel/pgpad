<script lang="ts">
	import TabBar from '$lib/components/ui/TabBar.svelte';
	import { tabs, type ScriptTab, type TableViewTab } from '$lib/stores/tabs.svelte';
	import { backend } from '$lib/backend';
	import { onDestroy, onMount } from 'svelte';

	// All tabs (scripts + table views)
	const allTabs = $derived(
		tabs.all.map((tab): { id: number; name: string } => {
			if (tab.type === 'script') {
				return {
					id: (tab as ScriptTab).scriptId,
					name: tab.title
				};
			} else {
				// table-view
				return {
					id: (tab as TableViewTab).tableTabId,
					name: `📋 ${tab.title}`
				};
			}
		})
	);

	const activeTabIdForTabBar = $derived.by((): number | null => {
		const activeTab = tabs.active;
		if (!activeTab) return null;
		if (activeTab.type === 'script') {
			return (activeTab as ScriptTab).scriptId;
		} else if (activeTab.type === 'table-view') {
			return (activeTab as TableViewTab).tableTabId;
		}
		return null;
	});

	function handleTabSelect(tabId: number) {
		// Find the tab by its numeric ID
		const tab = tabs.all.find((t) => {
			if (t.type === 'script') {
				return (t as ScriptTab).scriptId === tabId;
			} else if (t.type === 'table-view') {
				return (t as TableViewTab).tableTabId === tabId;
			}
			return false;
		});

		if (tab) {
			tabs.switchToTab(tab.id);
		}
	}

	function handleTabClose(tabId: number) {
		// Find the tab by its numeric ID
		const tab = tabs.all.find((t) => {
			if (t.type === 'script') {
				return (t as ScriptTab).scriptId === tabId;
			} else if (t.type === 'table-view') {
				return (t as TableViewTab).tableTabId === tabId;
			}
			return false;
		});

		if (tab) {
			tabs.closeTab(tab.id);
		}
	}

	function handleNewScript() {
		tabs.createNewScript();
	}

	function handleScriptRename(tabId: number, newName: string) {
		const tabIdStr = `script-${tabId}`;
		tabs.renameScript(tabIdStr, newName);
		// Table-view tabs can't be renamed
	}

	function getScriptStatus(tab: { id: number; name: string }): 'normal' | 'modified' | 'error' {
		// Find the actual tab
		const storeTab = tabs.all.find((t) => {
			if (t.type === 'script') {
				return (t as ScriptTab).scriptId === tab.id;
			} else if (t.type === 'table-view') {
				return (t as TableViewTab).tableTabId === tab.id;
			}
			return false;
		});
		return storeTab?.isDirty ? 'modified' : 'normal';
	}

	let unlistenNewTab: (() => void) | null = null;
	let unlistenCloseTab: (() => void) | null = null;
	onMount(async () => {
		unlistenNewTab = await backend.listen('new_tab', handleNewScript);
		unlistenCloseTab = await backend.listen('close_tab', () => {
			const activeId = activeTabIdForTabBar;
			if (activeId) {
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
	activeTabId={activeTabIdForTabBar}
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
