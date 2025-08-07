<script lang="ts">
	import TitleBar from '$lib/components/TitleBar.svelte';
	import Main from '$lib/components/Main.svelte';
	import '$lib/stores/theme';

	// Connection state for title bar
	let currentConnection: { name: string; connected: boolean } | null = $state(null);
	let isConnecting = $state(false);
</script>

<div class="flex h-screen flex-col overflow-hidden">
	<TitleBar {currentConnection} {isConnecting} />
	<div class="flex-1 overflow-hidden">
		<Main bind:currentConnection bind:isConnecting />
	</div>
</div>
