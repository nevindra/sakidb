<script lang="ts">
	import { Combobox as ComboboxPrimitive } from "bits-ui";
	import ChevronsUpDownIcon from "@lucide/svelte/icons/chevrons-up-down";
	import { cn, type WithoutChild } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	}: WithoutChild<ComboboxPrimitive.TriggerProps> = $props();
</script>

<ComboboxPrimitive.Trigger
	bind:ref
	data-slot="combobox-trigger"
	class={cn(
		"absolute top-1/2 end-2 -translate-y-1/2 text-muted-foreground",
		className
	)}
	{...restProps}
>
	{#if children}
		{@render children?.()}
	{:else}
		<ChevronsUpDownIcon class="size-4 opacity-50" />
	{/if}
</ComboboxPrimitive.Trigger>
