<script lang="ts">
	import * as Kbd from '$lib/components/ui/kbd';

	const isMacOS = window.__PGPAD_INTERNAL__?.platform === 'macos';
	const modKey = isMacOS ? '⌘' : 'Ctrl';
	const shiftKey = isMacOS ? '⇧' : 'Shift';
</script>

<div class="text-muted-foreground flex flex-1 items-center justify-center">
	<div class="space-y-3 text-xs">
		<div class="flex min-w-[20rem] items-center justify-between gap-4">
			<span class="text-muted-foreground/80">Run selected text or current line</span>
			<Kbd.Group>
				<Kbd.Root>{modKey}</Kbd.Root>
				<Kbd.Root>Enter</Kbd.Root>
			</Kbd.Group>
		</div>
		<div class="flex min-w-[20rem] items-center justify-between gap-4">
			<span class="text-muted-foreground/80">Run entire script</span>
			<Kbd.Group>
				<Kbd.Root>{modKey}</Kbd.Root>
				<Kbd.Root>R</Kbd.Root>
			</Kbd.Group>
		</div>
		<div class="flex min-w-[20rem] items-center justify-between gap-4">
			<span class="text-muted-foreground/80">Format SQL</span>
			<Kbd.Group>
				<Kbd.Root>{modKey}</Kbd.Root>
				<Kbd.Root>{shiftKey}</Kbd.Root>
				<Kbd.Root>F</Kbd.Root>
			</Kbd.Group>
		</div>
	</div>
</div>
