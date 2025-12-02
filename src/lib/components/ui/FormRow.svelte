<script lang="ts">
	interface Props {
		label: string;
		forId: string;
		description?: string;
		error?: string;
		required?: boolean;
		size?: 'sm' | 'md';
	}

	let {
		label,
		forId,
		description = undefined,
		error = undefined,
		required = false,
		size = 'md'
	}: Props = $props();

	const descId = $derived(`${forId}-desc`);
	const errId = $derived(`${forId}-err`);
	const describedby = $derived(description ? descId : undefined);
	const errorId = $derived(error ? errId : undefined);
	const spacingClass = $derived(size === 'sm' ? 'space-y-1' : 'space-y-2');
	const labelClass = $derived('text-foreground text-sm font-medium');
</script>

<div class={spacingClass}>
	<label for={forId} class={labelClass}>
		{label}{required ? ' *' : ''}
	</label>
	<div>
		<slot {describedby} {errorId}></slot>
	</div>
	{#if description}
		<p id={descId} class="text-muted-foreground text-xs">{description}</p>
	{/if}
	{#if error}
		<p id={errId} class="text-error text-xs">{error}</p>
	{/if}
</div>
