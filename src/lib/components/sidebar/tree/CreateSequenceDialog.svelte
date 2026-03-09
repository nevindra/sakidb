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
    connectionId,
    databaseName,
    oncreated,
  }: {
    open?: boolean;
    schema: string;
    connectionId: string;
    databaseName: string;
    oncreated?: () => void;
  } = $props();

  const app = getAppState();
  const engine = $derived(app.getSavedConnection(connectionId)?.engine as EngineType | undefined);
  const dialect = $derived(engine ? getDialect(engine) : null);

  let name = $state('');
  let startValue = $state(1);
  let increment = $state(1);
  let minValue = $state('');
  let maxValue = $state('');
  let cache = $state(1);
  let cycle = $state(false);
  let loading = $state(false);

  const createSql = $derived.by(() => {
    if (!dialect || !name) return '';
    const opts: { increment?: number; start?: number; min?: number; max?: number; cache?: number; cycle?: boolean } = {};
    if (startValue !== 1) opts.start = startValue;
    if (increment !== 1) opts.increment = increment;
    if (minValue !== '') opts.min = Number(minValue);
    if (maxValue !== '') opts.max = Number(maxValue);
    if (cache !== 1) opts.cache = cache;
    if (cycle) opts.cycle = true;
    return dialect.createSequence(schema, name, opts);
  });

  function resetForm() {
    name = '';
    startValue = 1;
    increment = 1;
    minValue = '';
    maxValue = '';
    cache = 1;
    cycle = false;
  }

  $effect(() => {
    if (open) {
      resetForm();
    }
  });

  async function handleCreate() {
    if (!createSql) return;
    loading = true;
    try {
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (!rid) return;
      await app.executeDdl(rid, createSql);
      open = false;
      resetForm();
      oncreated?.();
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
      <Dialog.Title>Create Sequence</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-3 py-2">
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="seq-name">Sequence Name</label>
        <Input id="seq-name" class="mt-1" bind:value={name} placeholder="my_sequence" />
      </div>
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="seq-start">Start Value</label>
          <Input id="seq-start" class="mt-1" type="number" bind:value={startValue} />
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="seq-increment">Increment</label>
          <Input id="seq-increment" class="mt-1" type="number" bind:value={increment} />
        </div>
      </div>
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="seq-min">Min Value</label>
          <Input id="seq-min" class="mt-1" type="number" bind:value={minValue} placeholder="Optional" />
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="seq-max">Max Value</label>
          <Input id="seq-max" class="mt-1" type="number" bind:value={maxValue} placeholder="Optional" />
        </div>
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="seq-cache">Cache</label>
        <Input id="seq-cache" class="mt-1" type="number" bind:value={cache} />
      </div>
      <label class="flex items-center gap-2 cursor-pointer">
        <Checkbox bind:checked={cycle} />
        <span class="text-xs text-muted-foreground">Cycle</span>
      </label>
      <DdlPreview sql={createSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (open = false)} disabled={loading}>Cancel</Button>
      <Button size="sm" onclick={handleCreate} disabled={!name || loading}>Execute</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
