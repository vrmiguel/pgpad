<script lang="ts">
	import TitleBar from '$lib/components/TitleBar.svelte';
	import Main from '$lib/components/Main.svelte';
	import '$lib/stores/theme';

	// State that flows up from Main for title bar display
	let currentConnection: { name: string; connected: boolean } | null = $state(null);
	let isConnecting = $state(false);
	let selectedConnection: string | null = $state(null);
	let hasUnsavedChanges = $state(false);

	// Action callbacks from Main component
	let runQueryCallback: (() => void) | null = $state(null);
	let saveScriptCallback: (() => void) | null = $state(null);
</script>

<div class="flex h-full flex-col overflow-hidden">
	<TitleBar
		{currentConnection}
		{isConnecting}
		{selectedConnection}
		{hasUnsavedChanges}
		onRunQuery={() => runQueryCallback?.()}
		onSaveScript={() => saveScriptCallback?.()}
	/>
	<div class="flex-1 overflow-hidden">
		<Main
			bind:currentConnection
			bind:isConnecting
			bind:selectedConnection
			bind:hasUnsavedChanges
			bind:runQueryCallback
			bind:saveScriptCallback
		/>
	</div>
</div>
