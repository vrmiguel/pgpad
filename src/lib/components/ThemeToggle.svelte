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
	class={`transition-all duration-200 {className}`}
	onclick={toggleTheme}
	title={$theme === 'light' ? 'Switch to dark mode' : 'Switch to light mode'}
>
	{#if $theme === 'light'}
		<Sun class="w-4 h-4" />
	{:else}
		<Moon class="w-4 h-4" />
	{/if}
</Button> 