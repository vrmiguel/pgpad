<script lang="ts">
	import { Sun, Moon } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import { theme, toggleTheme } from '$lib/stores/theme';

	// Optional props for styling
	interface Props {
		size?: 'sm' | 'default' | 'lg';
		variant?: 'default' | 'ghost' | 'outline';
		class?: string;
	}

	let { size = 'sm', variant = 'ghost', class: className = '' }: Props = $props();
</script>

<Button
	{variant}
	{size}
	class={`{className} transition-all duration-200`}
	onclick={toggleTheme}
	title={$theme === 'light' ? 'Switch to dark mode' : 'Switch to light mode'}
>
	{#if $theme === 'light'}
		<Sun class="h-4 w-4" />
	{:else}
		<Moon class="h-4 w-4" />
	{/if}
</Button>
