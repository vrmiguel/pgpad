<script lang="ts">
	import { Database, Minus, Square, X, Circle } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import ThemeToggle from './ThemeToggle.svelte';
	import { onMount } from 'svelte';
	import { Commands } from '$lib/commands.svelte';

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
		await Commands.minimizeWindow();
	}

	async function maximizeWindow() {
		await Commands.maximizeWindow();
	}

	async function closeWindow() {
		await Commands.closeWindow();
	}
</script>

<div class="bg-background border-border/50 flex h-8 items-center border-b select-none">
	{#if isMacOS}
		<!-- macOS-style controls (left side) - NOT draggable -->
		<div class="flex items-center gap-2 px-4">
			<!-- Close (red) -->
			<button
				class="group h-3 w-3 rounded-full bg-red-500 transition-colors hover:bg-red-600"
				onclick={closeWindow}
				title="Close"
			>
				<div
					class="flex h-full w-full items-center justify-center rounded-full opacity-0 transition-opacity group-hover:opacity-100"
				>
					<X class="h-2 w-2 text-red-900" />
				</div>
			</button>

			<!-- Minimize (yellow) -->
			<button
				class="group h-3 w-3 rounded-full bg-yellow-500 transition-colors hover:bg-yellow-600"
				onclick={minimizeWindow}
				title="Minimize"
			>
				<div
					class="flex h-full w-full items-center justify-center rounded-full opacity-0 transition-opacity group-hover:opacity-100"
				>
					<Minus class="h-2 w-2 text-yellow-900" />
				</div>
			</button>

			<!-- Maximize/Fullscreen (green) -->
			<button
				class="group h-3 w-3 rounded-full bg-green-500 transition-colors hover:bg-green-600"
				onclick={maximizeWindow}
				title="Maximize"
			>
				<div
					class="flex h-full w-full items-center justify-center rounded-full opacity-0 transition-opacity group-hover:opacity-100"
				>
					<Square class="h-1.5 w-1.5 text-green-900" />
				</div>
			</button>
		</div>

		<!-- Center section - App info and connection status - DRAGGABLE -->
		<div class="flex flex-1 items-center justify-center gap-3 px-4" data-tauri-drag-region>
			<div class="flex items-center gap-2" data-tauri-drag-region>
				<Database class="text-primary h-4 w-4" data-tauri-drag-region />
				<span class="text-foreground text-sm font-semibold" data-tauri-drag-region>pgpad</span>
			</div>

			<!-- Connection status -->
			{#if currentConnection}
				<div
					class="bg-muted/30 flex items-center gap-2 rounded-md px-2 py-1"
					data-tauri-drag-region
				>
					{#if isConnecting}
						<div class="bg-warning h-2 w-2 animate-pulse rounded-full" data-tauri-drag-region></div>
						<span class="text-muted-foreground text-xs" data-tauri-drag-region>Connecting...</span>
					{:else if currentConnection.connected}
						<div class="bg-success h-2 w-2 rounded-full" data-tauri-drag-region></div>
						<span class="text-foreground text-xs" data-tauri-drag-region
							>{currentConnection.name}</span
						>
					{:else}
						<div class="bg-muted-foreground/40 h-2 w-2 rounded-full" data-tauri-drag-region></div>
						<span class="text-muted-foreground text-xs" data-tauri-drag-region
							>{currentConnection.name} (disconnected)</span
						>
					{/if}
				</div>
			{:else}
				<div
					class="bg-muted/20 flex items-center gap-2 rounded-md px-2 py-1"
					data-tauri-drag-region
				>
					<Circle class="text-muted-foreground/50 h-2 w-2" data-tauri-drag-region />
					<span class="text-muted-foreground text-xs" data-tauri-drag-region>No connection</span>
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
		<div class="flex flex-1 items-center gap-3 px-4" data-tauri-drag-region>
			<div class="flex items-center gap-2" data-tauri-drag-region>
				<Database class="text-primary h-4 w-4" data-tauri-drag-region />
				<span class="text-foreground text-sm font-semibold" data-tauri-drag-region>pgpad</span>
			</div>

			<!-- Connection status -->
			{#if currentConnection}
				<div
					class="bg-muted/30 flex items-center gap-2 rounded-md px-2 py-1"
					data-tauri-drag-region
				>
					{#if isConnecting}
						<div class="bg-warning h-2 w-2 animate-pulse rounded-full" data-tauri-drag-region></div>
						<span class="text-muted-foreground text-xs" data-tauri-drag-region>Connecting...</span>
					{:else if currentConnection.connected}
						<div class="bg-success h-2 w-2 rounded-full" data-tauri-drag-region></div>
						<span class="text-foreground text-xs" data-tauri-drag-region
							>{currentConnection.name}</span
						>
					{:else}
						<div class="bg-muted-foreground/40 h-2 w-2 rounded-full" data-tauri-drag-region></div>
						<span class="text-muted-foreground text-xs" data-tauri-drag-region
							>{currentConnection.name} (disconnected)</span
						>
					{/if}
				</div>
			{:else}
				<div
					class="bg-muted/20 flex items-center gap-2 rounded-md px-2 py-1"
					data-tauri-drag-region
				>
					<Circle class="text-muted-foreground/50 h-2 w-2" data-tauri-drag-region />
					<span class="text-muted-foreground text-xs" data-tauri-drag-region>No connection</span>
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
