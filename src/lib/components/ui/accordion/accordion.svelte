<script lang="ts">
	interface Props {
		children?: any;
		class?: string;
	}

	let { children, class: className = '' }: Props = $props();
</script>

<div class="space-y-2 {className}">
	{@render children?.()}
</div>
