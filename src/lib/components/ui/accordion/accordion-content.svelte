<script lang="ts">
	interface Props {
		children?: any;
		class?: string;
	}

	let { children, class: className = '' }: Props = $props();
</script>

<div class="p-3 {className}">
	{@render children?.()}
</div>
