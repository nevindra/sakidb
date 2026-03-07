<script lang="ts">
	import type { HTMLInputAttributes, HTMLInputTypeAttribute } from "svelte/elements";
	import { cn, type WithElementRef } from "$lib/utils.js";

	type InputType = Exclude<HTMLInputTypeAttribute, "file">;

	type Props = WithElementRef<
		Omit<HTMLInputAttributes, "type"> &
			({ type: "file"; files?: FileList } | { type?: InputType; files?: undefined })
	>;

	let {
		ref = $bindable(null),
		value = $bindable(),
		type,
		files = $bindable(),
		class: className,
		"data-slot": dataSlot = "input",
		...restProps
	}: Props = $props();
</script>

{#if type === "file"}
	<input
		bind:this={ref}
		data-slot={dataSlot}
		class={cn(
			"selection:bg-primary/30 selection:text-foreground border-transparent placeholder:text-muted-foreground/60 flex h-9 w-full min-w-0 rounded-md border bg-transparent px-3 pt-1.5 text-sm font-medium transition-colors outline-none disabled:cursor-not-allowed disabled:opacity-40",
			"hover:bg-accent/10 focus-visible:border-ring/70 focus-visible:bg-input/10",
			"aria-invalid:border-destructive",
			className
		)}
		type="file"
		bind:files
		bind:value
		{...restProps}
	/>
{:else}
	<input
		bind:this={ref}
		data-slot={dataSlot}
		class={cn(
			"border-transparent bg-transparent selection:bg-primary/30 selection:text-foreground placeholder:text-muted-foreground/60 flex h-9 w-full min-w-0 rounded-md border px-3 py-1 text-base transition-colors outline-none disabled:cursor-not-allowed disabled:opacity-40 md:text-sm",
			"hover:bg-accent/10 focus-visible:border-ring/70 focus-visible:bg-input/10",
			"aria-invalid:border-destructive",
			className
		)}
		{type}
		bind:value
		{...restProps}
	/>
{/if}
