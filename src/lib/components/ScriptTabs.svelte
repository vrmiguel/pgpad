<script lang="ts">
	import TabBar from '$lib/components/ui/TabBar.svelte';
	import { tabs, type ScriptTab } from '$lib/stores/tabs.svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { onDestroy, onMount } from 'svelte';

	const scriptTabs = $derived(
		tabs.all
			.filter((tab) => tab.type === 'script')
			.map((tab) => {
				return {
					id: tab.scriptId,
					name: tab.title
				};
			})
	);

	const activeScriptId = $derived(
		tabs.active?.type === 'script' ? (tabs.active as ScriptTab).scriptId : null
	);

	function handleTabSelect(scriptId: number) {
		const tabId = `script-${scriptId}`;
		tabs.switchToTab(tabId);
	}

	function handleTabClose(scriptId: number) {
		const tabId = `script-${scriptId}`;
		tabs.closeTab(tabId);
	}

	function handleNewScript() {
		tabs.createNewScript();
	}

	function handleScriptRename(scriptId: number, newName: string) {
		const tabId = `script-${scriptId}`;
		tabs.renameScript(tabId, newName);
	}

	function getScriptStatus(tab: { id: number; name: string }): 'normal' | 'modified' | 'error' {
		const tabId = `script-${tab.id}`;
		const storeTab = tabs.all.find((t) => t.id === tabId);
		return storeTab?.isDirty ? 'modified' : 'normal';
	}

	let unlistenNewTab: UnlistenFn | null = null;
	let unlistenCloseTab: UnlistenFn | null = null;
	onMount(async () => {
		unlistenNewTab = await listen('new_tab', handleNewScript);
		unlistenCloseTab = await listen('close_tab', () => {
			if (activeScriptId) {
				handleTabClose(activeScriptId);
			}
		});
	});
	onDestroy(() => {
		unlistenNewTab?.();
		unlistenCloseTab?.();
	});
</script>

<TabBar
	tabs={scriptTabs}
	activeTabId={activeScriptId}
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
