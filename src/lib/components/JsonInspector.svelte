<script lang="ts">
	import JsonViewer from './JsonViewer.svelte';
	import { X, Copy, Check } from '@lucide/svelte';

	interface Props {
		selectedCellData: any | null;
		onClose?: () => void;
	}

	let { selectedCellData, onClose }: Props = $props();

	let copySuccess = $state(false);
	let windowElement = $state<HTMLDivElement>();
	let isDragging = $state(false);
	let dragOffset = { x: 0, y: 0 };
	let windowSize = { width: 450, height: 400 };
	let animationFrameId: number | null = null;

	const initialPosition = { x: 120, y: 80 };

	async function copyJsonValue(): Promise<void> {
		if (!jsonValueToDisplay) return;

		try {
			const jsonString = JSON.stringify(jsonValueToDisplay(), null, 2);
			await navigator.clipboard.writeText(jsonString);
			copySuccess = true;
			setTimeout(() => (copySuccess = false), 2000);
		} catch (err) {
			console.error('Failed to copy JSON: ', err);
		}
	}

	const jsonValueToDisplay = $derived(() => {
		if (!selectedCellData) return null;

		if (typeof selectedCellData !== 'object' || selectedCellData === null) {
			return null;
		}

		return selectedCellData;
	});

	const isJsonValue = $derived(() => {
		return jsonValueToDisplay() !== null;
	});

	function handleMouseDown(e: MouseEvent) {
		if (e.target === e.currentTarget || (e.target as HTMLElement).closest('.window-titlebar')) {
			isDragging = true;
			if (windowElement) {
				const rect = windowElement.getBoundingClientRect();
				dragOffset.x = e.clientX - rect.left;
				dragOffset.y = e.clientY - rect.top;
			}
		}
	}

	function handleWindowMouseMove(e: MouseEvent) {
		if (isDragging && windowElement) {
			if (animationFrameId) {
				cancelAnimationFrame(animationFrameId);
			}

			animationFrameId = requestAnimationFrame(() => {
				if (!isDragging || !windowElement) return;

				let newX = e.clientX - dragOffset.x;
				let newY = e.clientY - dragOffset.y;

				const rect = windowElement.getBoundingClientRect();
				const maxX = window.innerWidth - rect.width;
				const maxY = window.innerHeight - rect.height;
				newX = Math.max(0, Math.min(newX, maxX));
				newY = Math.max(0, Math.min(newY, maxY));

				windowElement.style.transform = `translate(${newX}px, ${newY}px)`;
			});
		}
	}

	function handleWindowMouseUp() {
		isDragging = false;
		if (animationFrameId) {
			cancelAnimationFrame(animationFrameId);
			animationFrameId = null;
		}
	}
</script>

<svelte:window onmousemove={handleWindowMouseMove} onmouseup={handleWindowMouseUp} />

<div
	bind:this={windowElement}
	class="border-border/60 glass-card fixed rounded-xl border shadow-2xl backdrop-blur-xl select-none"
	class:cursor-move={isDragging}
	class:animate-fade-in={isJsonValue()}
	style="left: 0; top: 0; transform: translate({initialPosition.x}px, {initialPosition.y}px); width: {windowSize.width}px; height: {windowSize.height}px; max-width: 80vw; z-index: 1000; display: {isJsonValue()
		? 'block'
		: 'none'};"
	onmousedown={handleMouseDown}
	role="dialog"
	aria-label="JSON Viewer"
	tabindex="-1"
>
	<div
		class="window-titlebar glass-subtle border-border/50 flex cursor-move items-center justify-between rounded-t-xl border-b px-2 py-1"
	>
		<div
			class="drag-handle flex-1 opacity-30 transition-opacity duration-200 select-none hover:opacity-60"
		></div>
		<div class="relative z-10 flex items-center gap-1">
			<button
				class="hover:bg-accent/80 focus-ring inline-flex h-6 w-6 items-center justify-center rounded transition-all duration-200 hover:scale-105"
				onclick={copyJsonValue}
				title="Copy JSON"
				type="button"
			>
				{#if copySuccess}
					<Check class="text-success h-3 w-3" />
				{:else}
					<Copy class="text-muted-foreground hover:text-foreground h-3 w-3" />
				{/if}
			</button>
			{#if onClose}
				<button
					class="hover:bg-error/10 hover:text-error focus-ring inline-flex h-6 w-6 items-center justify-center rounded transition-all duration-200 hover:scale-105"
					onclick={onClose}
					title="Close window"
					aria-label="Close window"
				>
					<X class="h-3 w-3" />
				</button>
			{/if}
		</div>
	</div>

	<div class="bg-card/50 flex-1 overflow-auto rounded-b-xl backdrop-blur-sm">
		<div class="p-2">
			<JsonViewer json={jsonValueToDisplay()} depth={3} />
		</div>
	</div>
</div>

<style>
	.drag-handle {
		position: relative;
		height: 16px;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.drag-handle::before {
		content: '';
		position: absolute;
		top: 50%;
		left: 0;
		right: 0;
		transform: translateY(-50%);
		height: 10px;
		background-image:
			radial-gradient(circle at 3px 2.5px, currentColor 1px, transparent 1px),
			radial-gradient(circle at 3px 7.5px, currentColor 1px, transparent 1px);
		background-size: 6px 10px;
		background-repeat: repeat-x;
		opacity: 0.4;
	}

	.window-titlebar {
		user-select: none;
		-webkit-user-select: none;
	}

	.glass-card {
		will-change: transform;
		resize: both;
		overflow: hidden;
		min-width: 300px;
		min-height: 200px;
	}

	.cursor-move {
		transition: none !important;
	}
</style>
