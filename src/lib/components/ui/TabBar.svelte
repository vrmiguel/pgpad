<script lang="ts">
	import { X, Plus, Circle } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';

	interface TabItem {
		id: string | number;
		name: string;
		[key: string]: any; // Allow additional properties
	}

	interface Props<T extends TabItem> {
		tabs: T[];
		activeTabId: string | number | null;
		onTabSelect: (tabId: string | number) => void;
		onTabClose?: (tabId: string | number) => void;
		onNewTab?: () => void;
		onTabRename?: (tabId: string | number, newName: string) => void;
		showCloseButton?: boolean;
		showNewTabButton?: boolean;
		allowRename?: boolean;
		getTabStatus?: (tab: T) => 'normal' | 'modified' | 'error';
		// Styling
		maxTabWidth?: string;
		variant?: 'default' | 'seamless';
		// Labels
		newTabLabel?: string;
		closeTabLabel?: string;
	}

	let {
		tabs,
		activeTabId,
		onTabSelect,
		onTabClose,
		onNewTab,
		onTabRename,
		showCloseButton = true,
		showNewTabButton = false,
		allowRename = false,
		getTabStatus,
		maxTabWidth = 'max-w-48',
		variant = 'default',
		newTabLabel = 'New Tab',
		closeTabLabel = 'Close tab'
	}: Props<TabItem> = $props();

	let editingTabId = $state<string | number | null>(null);
	let editingName = $state('');
	let nameInput = $state<HTMLInputElement>();

	function handleTabClick(tabId: string | number) {
		// Don't switch tabs while editing
		if (editingTabId === tabId) return;

		// Clear any ongoing editing when switching tabs
		if (editingTabId !== null) {
			editingTabId = null;
			editingName = '';
		}

		onTabSelect(tabId);
	}

	function handleTabClose(e: Event, tabId: string | number) {
		e.stopPropagation();
		onTabClose?.(tabId);
	}

	function startEditingName(tabId: string | number, currentName: string) {
		if (!allowRename) return;

		editingTabId = tabId;
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
		if (editingTabId === null) return;

		const trimmedName = editingName.trim();
		const currentTab = tabs.find((t) => t.id === editingTabId);

		if (trimmedName && trimmedName !== currentTab?.name) {
			onTabRename?.(editingTabId, trimmedName);
		}

		editingTabId = null;
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
			editingTabId = null;
			editingName = '';
		}
	}

	function getStatusIndicator(tab: TabItem) {
		if (!getTabStatus) return null;
		const status = getTabStatus(tab);

		switch (status) {
			case 'modified':
				return { icon: Circle, class: 'h-1.5 w-1.5 flex-shrink-0 fill-amber-500 text-amber-500' };
			case 'error':
				return { icon: Circle, class: 'h-1.5 w-1.5 flex-shrink-0 fill-red-500 text-red-500' };
			default:
				return null;
		}
	}
</script>

<div
	class="border-border/50 flex items-center overflow-hidden border-b {variant === 'seamless'
		? 'bg-transparent'
		: 'bg-background'}"
>
	<!-- Tab bar -->
	<div class="flex flex-1 overflow-x-auto">
		{#each tabs as tab (tab.id)}
			{@const statusIndicator = getStatusIndicator(tab)}
			<div
				class="group relative flex {maxTabWidth} min-w-0 items-center {activeTabId === tab.id
					? 'bg-card border-border border-x shadow-lg'
					: 'hover:bg-muted/60 bg-transparent'}"
			>
				<!-- Tab content area (clickable) -->
				<button
					type="button"
					class="relative flex min-w-0 flex-1 items-center gap-2 px-4 py-3 text-sm transition-all duration-200 {activeTabId ===
					tab.id
						? 'text-foreground font-medium'
						: 'text-muted-foreground hover:text-foreground'}"
					onclick={() => handleTabClick(tab.id)}
					ondblclick={() => startEditingName(tab.id, tab.name)}
				>
					<!-- Tab name -->
					{#if editingTabId === tab.id}
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
							{tab.name}
						</span>
					{/if}

					<!-- Status indicator -->
					{#if statusIndicator}
						{@const IconComponent = statusIndicator.icon}
						<IconComponent class={statusIndicator.class} />
					{/if}
				</button>

				{#if showCloseButton && onTabClose}
					<button
						type="button"
						class="hover:bg-destructive/10 hover:text-destructive mr-2 flex-shrink-0 rounded p-1 opacity-0 transition-all duration-200 group-hover:opacity-100 {activeTabId ===
						tab.id
							? 'text-muted-foreground'
							: ''}"
						onclick={(e) => handleTabClose(e, tab.id)}
						title={closeTabLabel}
					>
						<X class="h-3.5 w-3.5" />
					</button>
				{/if}

				<!-- Active tab indicator - spans entire tab width -->
				{#if activeTabId === tab.id}
					<div
						class="absolute right-0 bottom-0 left-0 h-1 bg-blue-500 transition-all duration-200"
					></div>
				{/if}
			</div>
		{/each}
	</div>

	{#if showNewTabButton && onNewTab}
		<Button
			variant="ghost"
			size="sm"
			class="border-border/30 hover:bg-muted/30 rounded-none border-l"
			onclick={onNewTab}
			title={newTabLabel}
		>
			<Plus class="h-4 w-4" />
		</Button>
	{/if}
</div>
