<script lang="ts">
	import { createJSONEditor } from 'vanilla-jsoneditor';
	import { onMount, onDestroy } from 'svelte';

	interface Props {
		json: any;
		depth?: number;
	}

	let { json, depth = 2 }: Props = $props();

	let container: HTMLDivElement;
	let editor: any = null;
	let expandedStrings = $state(new WeakMap<HTMLElement, boolean>());
	let clickHandlerSetup = false;
	let clickHandler: ((e: Event) => void) | null = null;

	onMount(() => {
		if (container && json !== undefined) {
			try {
				editor = createJSONEditor({
					target: container,
					props: {
						content: { json },
						readOnly: true,
						mainMenuBar: false,
						navigationBar: false,
						statusBar: false,
						mode: 'tree',
						onChange: () => {
							// Read only
						}
					}
				});

				if (container) {
					const mainElement = container.querySelector('.jse-main');
					if (mainElement) {
						(mainElement as HTMLElement).style.height = 'auto';
						(mainElement as HTMLElement).style.minHeight = 'auto';
						(mainElement as HTMLElement).style.maxHeight = 'none';
					}
				}

				// Auto-expand to specified depth
				if (editor && depth > 0) {
					setTimeout(() => {
						try {
							// Expand to the specified depth
							editor.expand([], (path: any[]) => path.length < depth);
						} catch (e) {
							// Ignore expansion errors
							console.debug('Could not expand JSON to depth:', e);
						}
					}, 100);
				}

				if (!clickHandlerSetup) {
					setupEventDelegation();
					clickHandlerSetup = true;
				}
			} catch (error) {
				console.error('Error initializing JSON editor:', error);
			}
		}
	});

	function setupEventDelegation() {
		if (!container) return;

		clickHandler = (e: Event) => {
			const target = e.target as HTMLElement;

			// Check if clicked element is a string value
			if (
				target &&
				target.classList.contains('jse-value') &&
				target.classList.contains('jse-string')
			) {
				e.stopPropagation();

				// Toggle expanded state using WeakMap (no memory leaks!)
				const isExpanded = expandedStrings.get(target) || false;

				if (isExpanded) {
					expandedStrings.delete(target);
					target.classList.remove('expanded');
				} else {
					expandedStrings.set(target, true);
					target.classList.add('expanded');
				}
			}
		};

		container.addEventListener('click', clickHandler);
	}

	onDestroy(() => {
		if (container && clickHandler) {
			container.removeEventListener('click', clickHandler);
			clickHandler = null;
		}

		if (editor) {
			try {
				editor.destroy();
			} catch (e) {
				console.debug('Error destroying JSON editor:', e);
			}
			editor = null;
		}

		expandedStrings = new WeakMap();
		clickHandlerSetup = false;
	});

	// React to prop changes
	$effect(() => {
		if (editor && json !== undefined) {
			try {
				editor.set({ json });
			} catch (e) {
				console.error('Error updating JSON viewer:', e);
			}
		}
	});
</script>

<div bind:this={container} class="json-viewer-container"></div>

<style>
	/* Compact JSON viewer for table cells */

	.json-viewer-container {
		font-family:
			ui-monospace, SFMono-Regular, 'SF Mono', Monaco, Consolas, 'Liberation Mono', 'Courier New',
			monospace;
		font-size: 11px !important;
		line-height: 1.2;
		/* Natural size with reasonable limits for huge JSON */
		min-height: auto;
		max-height: 150px;
		max-width: 100%;
		/* Smart overflow: only show scrollbar when needed */
		overflow: auto;
		/* Remove border to integrate seamlessly with table */
		border: none;
		border-radius: 0;
		background: transparent;
		padding: 0;
		margin: 0;
	}

	/* Override vanilla-jsoneditor to match table styling */
	:global(.json-viewer-container .jse-main) {
		background: transparent !important;
		border: none !important;
		padding: 0 !important;
		margin: 0 !important;
		font-size: 11px !important;
	}

	:global(.json-viewer-container .jse-contents) {
		background: transparent !important;
		color: hsl(var(--foreground)) !important;
		font-size: 11px !important;
		padding: 0 !important;
		margin: 0 !important;
	}

	/* Match tables' font size */
	:global(.json-viewer-container *) {
		font-size: 11px !important;
		line-height: 1.2 !important;
	}

	/* Key styling */
	:global(.json-viewer-container .jse-key) {
		color: hsl(var(--foreground)) !important;
		font-weight: 500;
	}

	/* Value styling - fix the string breaking issue */
	:global(.json-viewer-container .jse-value) {
		word-break: normal !important;
		white-space: nowrap !important;
		overflow: hidden !important;
		text-overflow: ellipsis !important;
		max-width: 200px !important;
		display: inline-block !important;
		vertical-align: top !important;
	}

	:global(.json-viewer-container .jse-value.jse-string) {
		color: hsl(142 76% 36%) !important;
		/* Prevent weird line breaks in long strings */
		white-space: nowrap !important;
		overflow: hidden !important;
		text-overflow: ellipsis !important;
		max-width: 180px !important;
		cursor: pointer !important;
		transition: all 0.2s ease !important;
		text-decoration: underline !important;
		text-decoration-color: transparent !important;
		text-underline-offset: 2px !important;
	}

	:global(.json-viewer-container .jse-value.jse-string:hover) {
		background: hsl(var(--accent)) !important;
		border-radius: 2px !important;
		text-decoration-color: hsl(142 76% 36%) !important;
	}

	:global(.json-viewer-container .jse-value.jse-string.expanded) {
		white-space: normal !important;
		max-width: 300px !important;
		overflow: visible !important;
		position: relative !important;
		z-index: 10 !important;
		background: hsl(var(--background)) !important;
		padding: 2px 4px !important;
		border: 1px solid hsl(var(--border)) !important;
		border-radius: 3px !important;
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15) !important;
	}

	:global(.json-viewer-container .jse-value.jse-number) {
		color: hsl(32 95% 44%) !important;
	}

	:global(.json-viewer-container .jse-value.jse-boolean) {
		color: hsl(221 83% 53%) !important;
	}

	:global(.json-viewer-container .jse-value.jse-null) {
		color: hsl(var(--muted-foreground)) !important;
		font-style: italic;
	}

	/* Compact expand/collapse buttons */
	:global(.json-viewer-container .jse-button) {
		padding: 1px 3px !important;
		margin: 0 2px 0 0 !important;
		background: transparent !important;
		border: none !important;
		color: hsl(var(--muted-foreground)) !important;
		min-width: auto !important;
		height: auto !important;
		font-size: 10px !important;
	}

	:global(.json-viewer-container .jse-button:hover) {
		background: hsl(var(--accent)) !important;
		color: hsl(var(--foreground)) !important;
	}

	:global(.json-viewer-container .jse-property) {
		margin: 0 !important;
		padding: 1px 0 !important;
	}

	:global(.json-viewer-container .jse-tree-mode) {
		border: none !important;
		padding: 0 !important;
		margin: 0 !important;
	}

	:global(.json-viewer-container .jse-object),
	:global(.json-viewer-container .jse-array) {
		margin: 0 !important;
		padding: 0 0 0 12px !important; /* Small indent for nesting */
	}

	:global(.json-viewer-container .jse-menu) {
		display: none !important;
	}

	:global(.json-viewer-container .jse-navigation-bar) {
		display: none !important;
	}

	:global(.json-viewer-container .jse-status-bar) {
		display: none !important;
	}

	/* Ensure no unwanted borders appear */
	:global(.json-viewer-container .jse-main *) {
		border: none !important;
		outline: none !important;
	}

	/* Fix unnecessary scrolling in the editor itself */
	:global(.json-viewer-container .jse-main) {
		height: auto !important;
		min-height: auto !important;
		max-height: none !important;
	}

	:global(.json-viewer-container .jse-contents) {
		height: auto !important;
		min-height: auto !important;
		max-height: none !important;
		overflow: visible !important;
	}
</style>
