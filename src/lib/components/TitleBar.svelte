<script lang="ts">
	import { Minus, Square, X, Play, Save } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import ThemeToggle from './ThemeToggle.svelte';
	import { Commands } from '$lib/commands.svelte';

	interface Props {
		currentConnection?: {
			name: string;
			connected: boolean;
		} | null;
		isConnecting?: boolean;
		selectedConnection?: string | null;
		hasUnsavedChanges?: boolean;
		onRunQuery?: () => void;
		onSaveScript?: () => void;
	}

	let {
		currentConnection,
		isConnecting = false,
		selectedConnection = null,
		hasUnsavedChanges = false,
		onRunQuery = () => {},
		onSaveScript = () => {}
	}: Props = $props();

	const isMacOS = window.__PGPAD_INTERNAL__?.platform === 'macos';

	async function minimizeWindow() {
		await Commands.minimizeWindow();
	}

	async function maximizeWindow() {
		await Commands.maximizeWindow();
	}

	async function closeWindow() {
		await Commands.closeWindow();
	}
</script>

<div
	class="border-sidebar-border flex h-[42px] items-center border-b"
	style="background-color: var(--titlebar-bg);"
	data-tauri-drag-region
>
	{#if isMacOS}
		<div class="flex flex-1 items-center justify-center gap-3 pr-4 pl-20" data-tauri-drag-region>
			<!-- Action buttons - NOT draggable -->
			<div class="flex items-center gap-1.5">
				<Button
					variant="ghost"
					size="sm"
					class="h-7 gap-1.5 rounded-md border-0 bg-black/5 px-2.5 text-xs font-medium transition-all duration-150 hover:bg-black/10 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-white/5 dark:hover:bg-white/10"
					disabled={!selectedConnection}
					onclick={onRunQuery}
					title="Run Query (Ctrl+R for full script, Ctrl+Enter for selection)"
				>
					<Play class="h-3 w-3" />
					Run Query
				</Button>
				<Button
					variant="ghost"
					size="sm"
					class="h-7 gap-1.5 rounded-md border-0 px-2.5 text-xs font-medium transition-all duration-150 {hasUnsavedChanges
						? 'bg-orange-100 text-orange-800 hover:bg-orange-200 dark:bg-orange-900/30 dark:text-orange-200 dark:hover:bg-orange-900/40'
						: 'bg-black/5 hover:bg-black/10 dark:bg-white/5 dark:hover:bg-white/10'}"
					onclick={onSaveScript}
				>
					<Save class="h-3 w-3" />
					Save Script{hasUnsavedChanges ? '*' : ''}
				</Button>
			</div>

			<!-- Connection status - draggable -->
			{#if currentConnection}
				<div
					class="flex items-center gap-2 rounded-md bg-black/5 px-2.5 py-1 dark:bg-white/5"
					data-tauri-drag-region
				>
					{#if isConnecting}
						<div
							class="h-1.5 w-1.5 animate-pulse rounded-full bg-amber-500"
							data-tauri-drag-region
						></div>
						<span class="text-muted-foreground text-xs font-medium" data-tauri-drag-region
							>Connecting...</span
						>
					{:else if currentConnection.connected}
						<div class="h-1.5 w-1.5 rounded-full bg-green-500" data-tauri-drag-region></div>
						<span class="text-foreground text-xs font-medium" data-tauri-drag-region
							>{currentConnection.name}</span
						>
					{:else}
						<div class="h-1.5 w-1.5 rounded-full bg-red-400" data-tauri-drag-region></div>
						<span class="text-muted-foreground text-xs font-medium" data-tauri-drag-region
							>{currentConnection.name}</span
						>
					{/if}
				</div>
			{:else}
				<div
					class="flex items-center gap-2 rounded-md bg-black/5 px-2.5 py-1 dark:bg-white/5"
					data-tauri-drag-region
				>
					<div class="h-1.5 w-1.5 rounded-full bg-gray-400" data-tauri-drag-region></div>
					<span class="text-muted-foreground text-xs font-medium" data-tauri-drag-region
						>No connection</span
					>
				</div>
			{/if}
		</div>

		<!-- Right section with theme toggle - NOT draggable on macOS -->
		<div class="flex items-center pr-4">
			<ThemeToggle size="sm" class="h-6 w-6 p-0" />
		</div>
	{:else}
		<!-- Windows/Linux controls (right side) -->
		<!-- Left section - App info, action buttons, and connection status -->
		<div class="flex flex-1 items-center gap-3 px-4" data-tauri-drag-region>
			<!-- Action buttons - NOT draggable -->
			<div class="flex items-center gap-1.5">
				<Button
					variant="ghost"
					size="sm"
					class="h-7 gap-1.5 rounded-md border-0 bg-black/5 px-2.5 text-xs font-medium transition-all duration-150 hover:bg-black/10 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-white/5 dark:hover:bg-white/10"
					disabled={!selectedConnection}
					onclick={onRunQuery}
					title="Run Query (Ctrl+R for full script, Ctrl+Enter for selection)"
				>
					<Play class="h-3 w-3" />
					Run Query
				</Button>
				<Button
					variant="ghost"
					size="sm"
					class="h-7 gap-1.5 rounded-md border-0 px-2.5 text-xs font-medium transition-all duration-150 {hasUnsavedChanges
						? 'bg-orange-100 text-orange-800 hover:bg-orange-200 dark:bg-orange-900/30 dark:text-orange-200 dark:hover:bg-orange-900/40'
						: 'bg-black/5 hover:bg-black/10 dark:bg-white/5 dark:hover:bg-white/10'}"
					onclick={onSaveScript}
				>
					<Save class="h-3 w-3" />
					Save Script{hasUnsavedChanges ? '*' : ''}
				</Button>
			</div>

			<!-- Connection status - draggable -->
			{#if currentConnection}
				<div
					class="flex items-center gap-2 rounded-md bg-black/5 px-2.5 py-1 dark:bg-white/5"
					data-tauri-drag-region
				>
					{#if isConnecting}
						<div
							class="h-1.5 w-1.5 animate-pulse rounded-full bg-amber-500"
							data-tauri-drag-region
						></div>
						<span class="text-muted-foreground text-xs font-medium" data-tauri-drag-region
							>Connecting...</span
						>
					{:else if currentConnection.connected}
						<div class="h-1.5 w-1.5 rounded-full bg-green-500" data-tauri-drag-region></div>
						<span class="text-foreground text-xs font-medium" data-tauri-drag-region
							>{currentConnection.name}</span
						>
					{:else}
						<div class="h-1.5 w-1.5 rounded-full bg-red-400" data-tauri-drag-region></div>
						<span class="text-muted-foreground text-xs font-medium" data-tauri-drag-region
							>{currentConnection.name}</span
						>
					{/if}
				</div>
			{:else}
				<div
					class="flex items-center gap-2 rounded-md bg-black/5 px-2.5 py-1 dark:bg-white/5"
					data-tauri-drag-region
				>
					<div class="h-1.5 w-1.5 rounded-full bg-gray-400" data-tauri-drag-region></div>
					<span class="text-muted-foreground text-xs font-medium" data-tauri-drag-region
						>No connection</span
					>
				</div>
			{/if}
		</div>

		<!-- Right section - Theme toggle and Window controls - NOT draggable -->
		<div class="flex items-center gap-1">
			<ThemeToggle size="sm" class="mr-1 h-7 w-7 p-0" />
			<Button
				variant="ghost"
				size="sm"
				class="hover:bg-muted/50 h-8 w-8 rounded-none p-0"
				onclick={minimizeWindow}
			>
				<Minus class="h-3 w-3" />
			</Button>

			<Button
				variant="ghost"
				size="sm"
				class="hover:bg-muted/50 h-8 w-8 rounded-none p-0"
				onclick={maximizeWindow}
			>
				<Square class="h-3 w-3" />
			</Button>

			<Button
				variant="ghost"
				size="sm"
				class="hover:bg-destructive hover:text-destructive-foreground h-8 w-8 rounded-none p-0"
				onclick={closeWindow}
			>
				<X class="h-3 w-3" />
			</Button>
		</div>
	{/if}
</div>
