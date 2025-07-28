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

<div class="border border-border/50 rounded-lg bg-background/50 {className}">
	<button
		class="flex items-center justify-between w-full p-3 text-left hover:bg-muted/50 transition-colors duration-200 rounded-lg"
		onclick={toggle}
	>
		<div class="flex items-center gap-2">
			{#if icon}
				{@const IconComponent = icon}
				<IconComponent class="w-4 h-4 text-muted-foreground" />
			{/if}
			<span class="text-sm font-medium text-foreground">{title}</span>
		</div>
		<ChevronDown 
			class="w-4 h-4 text-muted-foreground transition-transform duration-200 {open ? 'rotate-180' : ''}" 
		/>
	</button>
	
	{#if open}
		<div
			class="border-t border-border/30"
			transition:slide={{ duration: 300, easing: cubicOut }}
		>
			{@render children?.()}
		</div>
	{/if}
</div> 