<script lang="ts">
  import type { ColumnInfo } from '$lib/types';
  import { Columns3, Key } from '@lucide/svelte';

  let { columns }: { columns: ColumnInfo[] } = $props();
</script>

{#each columns as col (col.name)}
  <div class="pl-16 pr-2 py-0.5 text-[11px] text-muted-foreground flex items-center gap-1.5">
    {#if col.is_primary_key}
      <Key class="h-2.5 w-2.5 text-warning shrink-0" />
    {:else}
      <Columns3 class="h-2.5 w-2.5 text-text-dim shrink-0" />
    {/if}
    <span class="truncate">{col.name}</span>
    <span class="text-text-dim text-[10px] ml-auto shrink-0">{col.data_type}</span>
  </div>
{/each}
