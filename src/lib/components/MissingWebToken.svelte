<script lang="ts">
	import '$lib/stores/theme';
</script>

<main class="bg-background text-foreground grid min-h-screen place-items-center px-6 py-8">
	<section
		class="border-border bg-card text-card-foreground w-full max-w-lg rounded-md border p-6 shadow-xl"
	>
		<h1 class="mb-3 text-xl leading-snug font-semibold">pgpad-web needs a launch token</h1>
		<p class="text-muted-foreground mb-3 text-sm leading-6">
			Start pgpad-web from your terminal and open the full URL it prints, including the
			<code class="bg-muted text-foreground rounded-sm px-1.5 py-0.5 text-[13px]">?token=...</code>
			part.
		</p>
		<p class="text-muted-foreground text-sm leading-6">
			After pgpad stores the token for this browser tab, it will remove the token from the address
			bar.
		</p>
	</section>
</main>
