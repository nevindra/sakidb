<script lang="ts">
	import * as Popover from "$lib/components/ui/popover";
	import { Checkbox } from "$lib/components/ui/checkbox";
	import ChevronDownIcon from "@lucide/svelte/icons/chevron-down";
	import XIcon from "@lucide/svelte/icons/x";
	import { cn } from "$lib/utils.js";

	let {
		options = [],
		selected = $bindable([]),
		placeholder = "Select...",
		class: className,
	}: {
		options: string[];
		selected: string[];
		placeholder?: string;
		class?: string;
	} = $props();

	let open = $state(false);

	function toggle(option: string) {
		if (selected.includes(option)) {
			selected = selected.filter((s) => s !== option);
		} else {
			selected = [...selected, option];
		}
	}

	function remove(option: string) {
		selected = selected.filter((s) => s !== option);
	}

	function clear() {
		selected = [];
	}
</script>

<Popover.Root bind:open>
	<Popover.Trigger
		class={cn(
			"border-transparent data-[placeholder]:text-muted-foreground [&_svg:not([class*='text-'])]:text-muted-foreground focus-visible:border-ring/70 focus-visible:bg-input/10 flex w-full min-h-9 items-center justify-between gap-2 rounded-md border bg-transparent px-3 py-1.5 text-sm transition-colors outline-none select-none hover:bg-accent/10 disabled:cursor-not-allowed disabled:opacity-50",
			className
		)}
	>
		<div class="flex flex-wrap gap-1 flex-1">
			{#if selected.length === 0}
				<span class="text-muted-foreground/60">{placeholder}</span>
			{:else}
				{#each selected as item}
					<span class="inline-flex items-center gap-0.5 rounded bg-accent px-1.5 py-0.5 text-xs text-accent-foreground">
						{item}
						<!-- svelte-ignore a11y_click_events_have_key_events -->
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<span
							class="hover:text-destructive transition-colors cursor-pointer"
							onclick={(e) => { e.stopPropagation(); remove(item); }}
						>
							<XIcon class="size-3" />
						</span>
					</span>
				{/each}
			{/if}
		</div>
		<ChevronDownIcon class="size-4 opacity-50 shrink-0" />
	</Popover.Trigger>
	<Popover.Content class="w-(--bits-popover-anchor-width) p-1" align="start">
		{#if options.length === 0}
			<p class="text-xs text-muted-foreground py-2 text-center">No options</p>
		{:else}
			<div class="max-h-[200px] overflow-y-auto">
				{#each options as option}
					<button
						type="button"
						class="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent hover:text-accent-foreground transition-colors cursor-default"
						onclick={() => toggle(option)}
					>
						<Checkbox checked={selected.includes(option)} />
						<span class="truncate">{option}</span>
					</button>
				{/each}
			</div>
			{#if selected.length > 0}
				<div class="border-t border-border mt-1 pt-1">
					<button
						type="button"
						class="w-full text-xs text-muted-foreground hover:text-foreground py-1 transition-colors"
						onclick={clear}
					>
						Clear all
					</button>
				</div>
			{/if}
		{/if}
	</Popover.Content>
</Popover.Root>
