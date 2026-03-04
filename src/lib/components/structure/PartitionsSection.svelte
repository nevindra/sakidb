<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { StructureTab } from '$lib/types';
  import { Button } from '$lib/components/ui/button';
  import { Plus, Trash2 } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import DdlPreview from './DdlPreview.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { generateAddPartition, generateDetachPartition } from '$lib/utils/ddl';

  let { tab }: { tab: StructureTab } = $props();

  const app = getAppState();

  function formatRowCount(count: number | null): string {
    if (count === null) return '';
    if (count >= 1_000_000) return `~${(count / 1_000_000).toFixed(1)}M`;
    if (count >= 1_000) return `~${(count / 1_000).toFixed(1)}k`;
    return `~${count}`;
  }

  // ── Add partition dialog ──
  let addOpen = $state(false);
  let addName = $state('');
  let addForValues = $state('');
  let addLoading = $state(false);

  const addSql = $derived(
    addName && addForValues
      ? generateAddPartition(tab.schema, tab.table, {
          name: addName,
          forValues: addForValues,
        })
      : ''
  );

  async function handleAdd() {
    if (!addSql) return;
    addLoading = true;
    try {
      await app.executeDdl(tab.runtimeConnectionId, addSql);
      addOpen = false;
      addName = '';
      addForValues = '';
      app.loadStructureTab(tab.id);
    } catch {
      // Error shown via toast
    } finally {
      addLoading = false;
    }
  }

  // ── Detach partition ──
  let detachOpen = $state(false);
  let detachName = $state('');

  function confirmDetach(name: string) {
    detachName = name;
    detachOpen = true;
  }

  async function handleDetach() {
    const sql = generateDetachPartition(tab.schema, tab.table, detachName);
    try {
      await app.executeDdl(tab.runtimeConnectionId, sql);
      app.loadStructureTab(tab.id);
    } catch {
      // Error shown via toast
    }
  }
</script>

<div class="p-3">
  {#if tab.partitionInfo}
    <!-- Partition strategy -->
    <div class="flex items-center gap-3 mb-4">
      <Badge variant="outline" class="text-xs">{tab.partitionInfo.strategy}</Badge>
      <span class="text-xs text-muted-foreground">
        Partition key: <span class="font-mono text-foreground">{tab.partitionInfo.partition_key}</span>
      </span>
    </div>

    <!-- Partition list -->
    <table class="w-full text-xs">
      <thead>
        <tr class="text-left text-muted-foreground border-b border-border">
          <th class="py-1.5 px-2 font-medium">Partition Name</th>
          <th class="py-1.5 px-2 font-medium">Bound</th>
          <th class="py-1.5 px-2 font-medium">Rows</th>
          <th class="py-1.5 px-2 font-medium w-10"></th>
        </tr>
      </thead>
      <tbody>
        {#each tab.partitionInfo.partitions as part (part.name)}
          <tr class="border-b border-border/50 hover:bg-sidebar-accent/50 transition-colors">
            <td class="py-1.5 px-2 font-medium text-foreground">{part.name}</td>
            <td class="py-1.5 px-2 text-muted-foreground font-mono">{part.expression}</td>
            <td class="py-1.5 px-2 text-muted-foreground tabular-nums">{formatRowCount(part.row_count_estimate)}</td>
            <td class="py-1.5 px-2">
              <button
                class="p-0.5 text-muted-foreground hover:text-destructive transition-colors"
                onclick={() => confirmDetach(part.name)}
                title="Detach partition"
              >
                <Trash2 class="h-3 w-3" />
              </button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>

    {#if tab.partitionInfo.partitions.length === 0}
      <p class="text-xs text-muted-foreground py-4 text-center">No partitions defined</p>
    {/if}

    <div class="mt-3">
      <Button variant="outline" size="sm" class="h-7 text-xs" onclick={() => (addOpen = true)}>
        <Plus class="h-3 w-3 mr-1" />
        Add Partition
      </Button>
    </div>
  {:else}
    <p class="text-xs text-muted-foreground py-8 text-center">This table is not partitioned</p>
  {/if}
</div>

<!-- Add Partition Dialog -->
<Dialog.Root bind:open={addOpen}>
  <Dialog.Content class="max-w-md">
    <Dialog.Header>
      <Dialog.Title>Add Partition</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-3 py-2">
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="part-name">Partition Name</label>
        <input id="part-name" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addName} placeholder="{tab.table}_2024_q1" />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="part-values">FOR VALUES clause</label>
        <input id="part-values" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addForValues} placeholder="FROM ('2024-01-01') TO ('2024-04-01')" />
        <p class="text-[10px] text-muted-foreground mt-1">
          Examples: FROM ('2024-01-01') TO ('2024-04-01') | IN ('active', 'pending') | WITH (MODULUS 4, REMAINDER 0)
        </p>
      </div>
      <DdlPreview sql={addSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (addOpen = false)} disabled={addLoading}>Cancel</Button>
      <Button size="sm" onclick={handleAdd} disabled={!addName || !addForValues || addLoading}>Execute</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Detach Partition Confirm -->
<ConfirmDialog
  bind:open={detachOpen}
  title="Detach Partition"
  description={`Are you sure you want to detach partition "${detachName}"? The partition will become a standalone table.`}
  confirmLabel="Detach"
  variant="destructive"
  onconfirm={handleDetach}
/>
