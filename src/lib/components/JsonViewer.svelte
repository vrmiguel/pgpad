<script lang="ts">
	import type { Json } from '$lib/commands.svelte';
	import { SvelteSet } from 'svelte/reactivity';

	interface Props {
		json: Json;
		depth?: number;
	}

	let { json, depth = 2 }: Props = $props();

	// Holds paths that the user has expanded or collapsed
	// If a path is in the set as is, it is expanded
	// If a path is in the set as __collapsed, it is collapsed
	//
	// I tried using a Map<string, boolean> but for some reason it didn't work
	let userOverrides = new SvelteSet<string>();
	let expandedStrings = new SvelteSet<string>();

	function getValueType(value: Json): string {
		if (value === null) return 'null';
		if (typeof value === 'boolean') return 'boolean';
		if (typeof value === 'number') return 'number';
		if (typeof value === 'string') return 'string';
		if (Array.isArray(value)) return 'array';
		if (typeof value === 'object') return 'object';
		return 'unknown';
	}

	function isExpanded(path: string): boolean {
		return userOverrides.has(path);
	}

	function toggleExpanded(path: string): void {
		const currentPath = path;
		const pathParts = currentPath.split('.').filter(Boolean);
		const depth = pathParts.length;
		const isUserCollapsed = userOverrides.has(path + '__collapsed');
		const isCurrentlyExpanded =
			!isUserCollapsed && (userOverrides.has(path) || shouldAutoExpand(depth));

		if (isCurrentlyExpanded) {
			// Currently expanded -> collapse it
			// For auto-expanded items, we add them to expanded Set as "collapsed" marker
			if (shouldAutoExpand(depth) && !userOverrides.has(path)) {
				// This is an auto-expanded item, mark it as user-collapsed
				userOverrides.add(path + '__collapsed');
			} else {
				// This was user-expanded, remove it
				userOverrides.delete(path);
			}
		} else {
			// Currently collapsed -> expand it
			if (userOverrides.has(path + '__collapsed')) {
				// Remove the collapsed marker
				userOverrides.delete(path + '__collapsed');
			} else {
				// Add to expanded
				userOverrides.add(path);
			}
		}
	}

	function toggleStringExpanded(path: string): void {
		if (expandedStrings.has(path)) {
			expandedStrings.delete(path);
		} else {
			expandedStrings.add(path);
		}
	}

	function shouldAutoExpand(currentDepth: number): boolean {
		return currentDepth < depth;
	}

	function renderValue(
		value: Json,
		key: string | number | null = null,
		path = '',
		currentDepth = 0
	) {
		const valueType = getValueType(value);
		const currentPath = path ? `${path}.${key}` : String(key || '');

		const isUserCollapsed = userOverrides.has(currentPath + '__collapsed');
		const shouldExpand =
			!isUserCollapsed && (isExpanded(currentPath) || shouldAutoExpand(currentDepth));

		if (valueType === 'object' && value && typeof value === 'object' && !Array.isArray(value)) {
			const objectValue = value as { [key: string]: Json };
			const keys = Object.keys(objectValue);
			const isEmpty = keys.length === 0;

			return {
				type: 'object',
				key,
				value: objectValue,
				path: currentPath,
				isEmpty,
				shouldExpand,
				keys: shouldExpand ? keys : undefined,
				currentDepth
			};
		}

		if (valueType === 'array' && Array.isArray(value)) {
			const arrayValue = value as Json[];
			const isEmpty = arrayValue.length === 0;

			return {
				type: 'array',
				key,
				value: arrayValue,
				path: currentPath,
				isEmpty,
				shouldExpand,
				items: shouldExpand ? arrayValue : undefined,
				currentDepth
			};
		}

		return {
			type: valueType,
			key,
			value,
			path: currentPath,
			currentDepth
		};
	}

	let renderedJson = $derived(renderValue(json));
</script>

<div class="json-viewer-container">
	<div class="json-content">
		{@render jsonElement(renderedJson)}
	</div>
</div>

{#snippet jsonElement(item: ReturnType<typeof renderValue>)}
	<div class="json-item" style="margin-left: {item.currentDepth * 12}px">
		{#if item.type === 'object'}
			<div class="json-line">
				{#if item.key !== null}
					<span class="json-key">"{item.key}"</span>
					<span class="json-colon">: </span>
				{/if}
				<button
					class="json-bracket clickable"
					onclick={() => toggleExpanded(item.path)}
					aria-label={item.shouldExpand ? 'Collapse object' : 'Expand object'}
					type="button"
				>
					&#123;
				</button>
				{#if item.isEmpty}
					<span class="json-bracket">&#125;</span>
				{:else if !item.shouldExpand}
					<span class="json-ellipsis">...</span>
					<span class="json-bracket">&#125;</span>
				{/if}
			</div>
			{#if item.shouldExpand && !item.isEmpty && item.keys}
				{#each item.keys as key, index (key)}
					<div style="margin-left: {(item.currentDepth + 1) * 12}px">
						<span class="json-property">
							{@render jsonElement(
								renderValue(item.value[key], key, item.path, item.currentDepth + 1)
							)}
							{#if index < item.keys.length - 1}<span class="json-comma">,</span>{/if}
						</span>
					</div>
				{/each}
				<div class="json-line" style="margin-left: {item.currentDepth * 12}px">
					<span class="json-bracket">&#125;</span>
				</div>
			{/if}
		{:else if item.type === 'array'}
			<div class="json-line">
				{#if item.key !== null}
					<span class="json-key">"{item.key}"</span>
					<span class="json-colon">: </span>
				{/if}
				<button
					class="json-bracket clickable"
					onclick={() => toggleExpanded(item.path)}
					aria-label={item.shouldExpand ? 'Collapse array' : 'Expand array'}
					type="button"
				>
					&#91;
				</button>
				{#if item.isEmpty}
					<span class="json-bracket">&#93;</span>
				{:else if !item.shouldExpand}
					<span class="json-ellipsis">...</span>
					<span class="json-bracket">&#93;</span>
				{/if}
			</div>
			{#if item.shouldExpand && !item.isEmpty && item.items}
				{#each item.items as arrayItem, index (index)}
					<div style="margin-left: {(item.currentDepth + 1) * 12}px">
						<span class="json-property">
							{@render jsonElement(renderValue(arrayItem, index, item.path, item.currentDepth + 1))}
							{#if index < item.items.length - 1}<span class="json-comma">,</span>{/if}
						</span>
					</div>
				{/each}
				<div class="json-line" style="margin-left: {item.currentDepth * 12}px">
					<span class="json-bracket">&#93;</span>
				</div>
			{/if}
		{:else}
			<!-- Inline value -->
			<span class="json-inline-property">
				{#if item.key !== null && typeof item.key === 'string'}
					<span class="json-key">"{item.key}"</span>
					<span class="json-colon">: </span>
				{/if}
				<span class="json-value json-{item.type}">
					{#if item.type === 'string' && typeof item.value === 'string'}
						{@const stringPath = `${item.path}_string`}
						{@const isStringExpanded = expandedStrings.has(stringPath)}
						{@const shouldTruncate = item.value.length > 30}

						{#if shouldTruncate && !isStringExpanded}
							<button
								class="json-string-value clickable"
								onclick={() => toggleStringExpanded(stringPath)}
								title="Click to expand full string"
								type="button"
							>
								"{item.value.slice(0, 30)}…"
							</button>
						{:else if shouldTruncate && isStringExpanded}
							<button
								class="json-string-value expanded"
								onclick={() => toggleStringExpanded(stringPath)}
								title="Click to collapse string"
								type="button"
							>
								"{item.value}"
							</button>
						{:else}
							<span class="json-string-value">"{item.value}"</span>
						{/if}
					{:else}
						{JSON.stringify(item.value)}
					{/if}
				</span>
			</span>
		{/if}
	</div>
{/snippet}

<style>
	.json-viewer-container {
		font-family:
			ui-monospace, SFMono-Regular, 'SF Mono', Monaco, Consolas, 'Liberation Mono', 'Courier New',
			monospace;
		font-size: 11px;
		line-height: 1.4;
		height: 100%;
		max-width: 100%;
		overflow-y: auto;
		overflow-x: hidden; /* no horizontal scrolling */
		border: none;
		border-radius: 0;
		background: transparent;
		padding: 0;
		margin: 0;
	}

	.json-content {
		padding: 2px;
		color: hsl(var(--foreground));
		min-width: 0; /* allow content to shrink */
	}

	.json-item {
		margin: 0;
		min-width: 0; /* allow content to shrink */
	}

	.json-line {
		display: flex;
		align-items: center;
		margin: 1px 0;
		line-height: 1.4;
		min-width: 0; /* allow content to shrink */
	}

	.json-property {
		display: inline-flex;
		align-items: center;
		min-width: 0; /* allow content to shrink */
	}

	.json-inline-property {
		display: inline-flex;
		align-items: center;
		min-width: 0; /* Allow content to shrink */
	}

	.json-key {
		color: hsl(var(--foreground));
		font-weight: 500;
		margin-right: 2px;
		flex-shrink: 0; /* don't shrink the key */
	}

	.json-colon {
		color: hsl(var(--foreground));
		margin-right: 4px;
		flex-shrink: 0; /* don't shrink the colon */
	}

	.json-bracket {
		color: hsl(var(--muted-foreground));
		font-weight: 500;
		background: transparent;
		border: none;
		padding: 0;
		margin: 0;
		font-family: inherit;
		font-size: inherit;
		flex-shrink: 0; /* don't shrink brackets */
	}

	.json-bracket.clickable {
		cursor: pointer;
		transition: color 0.2s ease;
		border-radius: 2px;
		padding: 1px 2px;
	}

	.json-bracket.clickable:hover {
		color: hsl(var(--foreground));
		background: hsl(var(--accent));
	}

	.json-bracket.clickable:focus {
		outline: 1px solid hsl(var(--ring));
		outline-offset: 1px;
	}

	.json-comma {
		color: hsl(var(--muted-foreground));
		margin-left: 0;
		flex-shrink: 0; /* don't shrink commas */
	}

	.json-ellipsis {
		color: hsl(var(--muted-foreground));
		font-style: italic;
		margin: 0 2px;
		user-select: none;
	}

	/* Value type styling */
	.json-value {
		word-break: break-word;
		min-width: 0; /* allow values to shrink */
		flex: 1; /* allow values to take remaining space */
	}

	.json-string {
		color: hsl(142 76% 36%);
	}

	.json-number {
		color: hsl(32 95% 44%);
	}

	.json-boolean {
		color: hsl(221 83% 53%);
	}

	.json-null {
		color: hsl(var(--muted-foreground));
		font-style: italic;
	}

	.json-string-value {
		background: transparent;
		border: none;
		padding: 0;
		margin: 0;
		font-family: inherit;
		font-size: inherit;
		/* TODO(vini): rethink colors */
		color: hsl(142 76% 36%);
		cursor: inherit;
		min-width: 0; /* allow strings to shrink */
		max-width: 100%; /* don't exceed container */
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		display: inline-block;
	}

	.json-string-value.clickable {
		cursor: pointer;
		text-decoration: underline;
		text-decoration-color: transparent;
		transition: all 0.2s ease;
	}

	.json-string-value.clickable:hover {
		text-decoration-color: hsl(142 76% 36%);
		background: hsl(var(--accent));
		border-radius: 2px;
		padding: 1px 2px;
		margin: -1px -2px;
	}

	.json-string-value.expanded {
		background: hsl(var(--accent));
		border-radius: 3px;
		padding: 2px 4px;
		margin: -2px -4px;
		position: relative;
		z-index: 10;
		cursor: pointer;
		white-space: normal; /* allow wrapping when expanded */
		max-width: none; /* remove width limit when expanded */
		overflow: visible; /* show full content when expanded */
	}
</style>
