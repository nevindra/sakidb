<script lang="ts">
  import { getAppState } from '$lib/stores';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import DdlPreview from '../../structure/DdlPreview.svelte';
  import { getDialect } from '$lib/dialects';
  import type { EngineType } from '$lib/types';

  let {
    open = $bindable(false),
    schema,
    sequenceName,
    connectionId,
    databaseName,
    onedited,
  }: {
    open?: boolean;
    schema: string;
    sequenceName: string;
    connectionId: string;
    databaseName: string;
    onedited?: () => void;
  } = $props();

  const app = getAppState();
  const engine = $derived(app.getSavedConnection(connectionId)?.engine as EngineType | undefined);
  const dialect = $derived(engine ? getDialect(engine) : null);

  let increment = $state('');
  let minValue = $state('');
  let maxValue = $state('');
  let cache = $state('');
  let cycle = $state(false);
  let restart = $state('');
  let loading = $state(false);

  const alterOpts = $derived.by(() => {
    const opts: { increment?: number; min?: number; max?: number; cache?: number; cycle?: boolean; restart?: number } = {};
    if (increment) opts.increment = Number(increment);
    if (minValue) opts.min = Number(minValue);
    if (maxValue) opts.max = Number(maxValue);
    if (cache) opts.cache = Number(cache);
    if (cycle) opts.cycle = true;
    if (restart) opts.restart = Number(restart);
    return opts;
  });

  const editSql = $derived(
    dialect && sequenceName
      ? dialect.alterSequence(schema, sequenceName, alterOpts)
      : ''
  );

  function resetForm() {
    increment = '';
    minValue = '';
    maxValue = '';
    cache = '';
    cycle = false;
    restart = '';
  }

  $effect(() => {
    if (open) {
      resetForm();
    }
  });

  async function handleEdit() {
    if (!editSql) return;
    loading = true;
    try {
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (!rid) return;
      await app.executeDdl(rid, editSql);
      open = false;
      onedited?.();
    } catch {
      // Error handled by store
    } finally {
      loading = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-md">
    <Dialog.Header>
      <Dialog.Title>Edit Sequence</Dialog.Title>
      <Dialog.Description>
        Alter <span class="font-mono text-foreground">{schema ? `"${schema}".` : ''}"{sequenceName}"</span>
      </Dialog.Description>
    </Dialog.Header>
    <div class="space-y-3 py-2">
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="seq-increment">Increment</label>
          <Input id="seq-increment" class="mt-1" type="number" bind:value={increment} placeholder="1" />
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="seq-min">Min Value</label>
          <Input id="seq-min" class="mt-1" type="number" bind:value={minValue} placeholder="1" />
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="seq-max">Max Value</label>
          <Input id="seq-max" class="mt-1" type="number" bind:value={maxValue} placeholder="2147483647" />
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="seq-cache">Cache</label>
          <Input id="seq-cache" class="mt-1" type="number" bind:value={cache} placeholder="1" />
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="seq-restart">Restart With</label>
          <Input id="seq-restart" class="mt-1" type="number" bind:value={restart} placeholder="" />
        </div>
        <div class="flex items-end pb-1">
          <label class="flex items-center gap-2 text-xs font-medium text-muted-foreground">
            <Checkbox bind:checked={cycle} />
            Cycle
          </label>
        </div>
      </div>
      <DdlPreview sql={editSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (open = false)} disabled={loading}>Cancel</Button>
      <Button size="sm" onclick={handleEdit} disabled={!editSql || loading}>
        {loading ? 'Executing...' : 'Execute'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
