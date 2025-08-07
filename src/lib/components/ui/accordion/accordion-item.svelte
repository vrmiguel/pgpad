<script lang="ts">
	import { ChevronDown } from '@lucide/svelte';
	import { slide } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';

	interface Props {
		title: string;
		icon?: any;
		open?: boolean;
		children?: any;
		class?: string;
	}

	let { title, icon, open = $bindable(false), children, class: className = '' }: Props = $props();

	function toggle() {
		open = !open;
	}
</script>

<div class="border-border/50 bg-background/50 rounded-lg border {className}">
	<button
		class="hover:bg-muted/50 flex w-full items-center justify-between rounded-lg p-3 text-left transition-colors duration-200"
		onclick={toggle}
	>
		<div class="flex items-center gap-2">
			{#if icon}
				{@const IconComponent = icon}
				<IconComponent class="text-muted-foreground h-4 w-4" />
			{/if}
			<span class="text-foreground text-sm font-medium">{title}</span>
		</div>
		<ChevronDown
			class="text-muted-foreground h-4 w-4 transition-transform duration-200 {open
				? 'rotate-180'
				: ''}"
		/>
	</button>

	{#if open}
		<div class="border-border/30 border-t" transition:slide={{ duration: 300, easing: cubicOut }}>
			{@render children?.()}
		</div>
	{/if}
</div>
