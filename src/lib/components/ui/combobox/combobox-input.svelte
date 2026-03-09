<script lang="ts">
	import { Combobox as ComboboxPrimitive } from "bits-ui";
	import { cn, type WithoutChild } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: WithoutChild<ComboboxPrimitive.InputProps> = $props();
</script>

<ComboboxPrimitive.Input
	bind:ref
	data-slot="combobox-input"
	class={cn(
		"border-transparent bg-transparent selection:bg-primary/30 selection:text-foreground placeholder:text-muted-foreground/60 flex h-9 w-full min-w-0 rounded-md border px-3 py-1 text-base transition-colors outline-none disabled:cursor-not-allowed disabled:opacity-40 md:text-sm",
		"hover:bg-accent/10 focus-visible:border-ring/70 focus-visible:bg-input/10",
		className
	)}
	{...restProps}
/>
