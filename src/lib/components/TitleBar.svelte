<script lang="ts">
	import { Database, Minus, Square, X, Circle } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import ThemeToggle from './ThemeToggle.svelte';
	import { onMount } from 'svelte';

	interface Props {
		currentConnection?: {
			name: string;
			connected: boolean;
		} | null;
		isConnecting?: boolean;
	}

	let { currentConnection, isConnecting = false }: Props = $props();

	let isMacOS = $state(false);

	onMount(() => {
		// TODO(vini): convert this to backend command
		isMacOS =
			navigator.platform.toLowerCase().includes('mac') || navigator.userAgent.includes('Mac');
	});

	// Window controls
	async function minimizeWindow() {
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		getCurrentWindow().minimize();
	}

	async function maximizeWindow() {
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		getCurrentWindow().toggleMaximize();
	}

	async function closeWindow() {
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		getCurrentWindow().close();
	}
</script>

<div class="flex items-center h-8 bg-background border-b border-border/50 select-none">
	{#if isMacOS}
		<!-- macOS-style controls (left side) - NOT draggable -->
		<div class="flex items-center gap-2 px-4">
			<!-- Close (red) -->
			<button
				class="w-3 h-3 rounded-full bg-red-500 hover:bg-red-600 transition-colors group"
				onclick={closeWindow}
				title="Close"
			>
				<div class="w-full h-full rounded-full opacity-0 group-hover:opacity-100 flex items-center justify-center transition-opacity">
					<X class="w-2 h-2 text-red-900" />
				</div>
			</button>
			
			<!-- Minimize (yellow) -->
			<button
				class="w-3 h-3 rounded-full bg-yellow-500 hover:bg-yellow-600 transition-colors group"
				onclick={minimizeWindow}
				title="Minimize"
			>
				<div class="w-full h-full rounded-full opacity-0 group-hover:opacity-100 flex items-center justify-center transition-opacity">
					<Minus class="w-2 h-2 text-yellow-900" />
				</div>
			</button>
			
			<!-- Maximize/Fullscreen (green) -->
			<button
				class="w-3 h-3 rounded-full bg-green-500 hover:bg-green-600 transition-colors group"
				onclick={maximizeWindow}
				title="Maximize"
			>
				<div class="w-full h-full rounded-full opacity-0 group-hover:opacity-100 flex items-center justify-center transition-opacity">
					<Square class="w-1.5 h-1.5 text-green-900" />
				</div>
			</button>
		</div>

		<!-- Center section - App info and connection status - DRAGGABLE -->
		<div class="flex items-center gap-3 px-4 flex-1 justify-center" data-tauri-drag-region>
			<div class="flex items-center gap-2" data-tauri-drag-region>
				<Database class="w-4 h-4 text-primary" data-tauri-drag-region />
				<span class="text-sm font-semibold text-foreground" data-tauri-drag-region>pgpad</span>
			</div>
			
			<!-- Connection status -->
			{#if currentConnection}
				<div class="flex items-center gap-2 px-2 py-1 rounded-md bg-muted/30" data-tauri-drag-region>
					{#if isConnecting}
						<div class="w-2 h-2 rounded-full bg-warning animate-pulse" data-tauri-drag-region></div>
						<span class="text-xs text-muted-foreground" data-tauri-drag-region>Connecting...</span>
					{:else if currentConnection.connected}
						<div class="w-2 h-2 rounded-full bg-success" data-tauri-drag-region></div>
						<span class="text-xs text-foreground" data-tauri-drag-region>{currentConnection.name}</span>
					{:else}
						<div class="w-2 h-2 rounded-full bg-muted-foreground/40" data-tauri-drag-region></div>
						<span class="text-xs text-muted-foreground" data-tauri-drag-region>{currentConnection.name} (disconnected)</span>
					{/if}
				</div>
			{:else}
				<div class="flex items-center gap-2 px-2 py-1 rounded-md bg-muted/20" data-tauri-drag-region>
					<Circle class="w-2 h-2 text-muted-foreground/50" data-tauri-drag-region />
					<span class="text-xs text-muted-foreground" data-tauri-drag-region>No connection</span>
				</div>
			{/if}
		</div>

		<!-- Right section with theme toggle - NOT draggable on macOS -->
		<div class="flex items-center pr-4">
			<ThemeToggle size="sm" class="h-6 w-6 p-0" />
		</div>
	{:else}
		<!-- Windows/Linux controls (right side) -->
		<!-- Left section - App info - DRAGGABLE -->
		<div class="flex items-center gap-3 px-4 flex-1" data-tauri-drag-region>
			<div class="flex items-center gap-2" data-tauri-drag-region>
				<Database class="w-4 h-4 text-primary" data-tauri-drag-region />
				<span class="text-sm font-semibold text-foreground" data-tauri-drag-region>pgpad</span>
			</div>
			
			<!-- Connection status -->
			{#if currentConnection}
				<div class="flex items-center gap-2 px-2 py-1 rounded-md bg-muted/30" data-tauri-drag-region>
					{#if isConnecting}
						<div class="w-2 h-2 rounded-full bg-warning animate-pulse" data-tauri-drag-region></div>
						<span class="text-xs text-muted-foreground" data-tauri-drag-region>Connecting...</span>
					{:else if currentConnection.connected}
						<div class="w-2 h-2 rounded-full bg-success" data-tauri-drag-region></div>
						<span class="text-xs text-foreground" data-tauri-drag-region>{currentConnection.name}</span>
					{:else}
						<div class="w-2 h-2 rounded-full bg-muted-foreground/40" data-tauri-drag-region></div>
						<span class="text-xs text-muted-foreground" data-tauri-drag-region>{currentConnection.name} (disconnected)</span>
					{/if}
				</div>
			{:else}
				<div class="flex items-center gap-2 px-2 py-1 rounded-md bg-muted/20" data-tauri-drag-region>
					<Circle class="w-2 h-2 text-muted-foreground/50" data-tauri-drag-region />
					<span class="text-xs text-muted-foreground" data-tauri-drag-region>No connection</span>
				</div>
			{/if}
		</div>

		<!-- Right section - Theme toggle and Window controls - NOT draggable -->
		<div class="flex items-center gap-1">
			<ThemeToggle size="sm" class="h-7 w-7 p-0 mr-1" />
			<Button
				variant="ghost"
				size="sm"
				class="h-8 w-8 p-0 hover:bg-muted/50 rounded-none"
				onclick={minimizeWindow}
			>
				<Minus class="w-3 h-3" />
			</Button>
			
			<Button
				variant="ghost"
				size="sm"
				class="h-8 w-8 p-0 hover:bg-muted/50 rounded-none"
				onclick={maximizeWindow}
			>
				<Square class="w-3 h-3" />
			</Button>
			
			<Button
				variant="ghost"
				size="sm"
				class="h-8 w-8 p-0 hover:bg-destructive hover:text-destructive-foreground rounded-none"
				onclick={closeWindow}
			>
				<X class="w-3 h-3" />
			</Button>
		</div>
	{/if}
</div> 