<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import * as RadioGroup from '$lib/components/ui/radio-group';
  import { invoke } from '@tauri-apps/api/core';
  import { getAppState } from '$lib/stores';

  let {
    open = $bindable(false),
    schema,
    tableName,
    connectionId,
    databaseName,
    onDuplicated,
  }: {
    open?: boolean;
    schema: string;
    tableName: string;
    connectionId: string;
    databaseName: string;
    onDuplicated?: () => void;
  } = $props();

  const app = getAppState();

  let newName = $state('');
  let mode = $state('structure');
  let loading = $state(false);

  $effect(() => {
    if (open) {
      newName = `${tableName}_copy`;
      mode = 'structure';
    }
  });

  async function handleDuplicate() {
    if (!newName.trim()) return;
    loading = true;
    try {
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (!rid) throw new Error('Not connected');

      const sql =
        mode === 'structure'
          ? `CREATE TABLE "${schema}"."${newName}" (LIKE "${schema}"."${tableName}" INCLUDING ALL);`
          : `CREATE TABLE "${schema}"."${newName}" AS SELECT * FROM "${schema}"."${tableName}";`;

      await invoke('execute_batch', { activeConnectionId: rid, sql });
      open = false;
      onDuplicated?.();
    } catch (e) {
      app.clearError();
      // Re-set so toast picks it up through the store
      throw e;
    } finally {
      loading = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content showCloseButton={false} class="max-w-sm">
    <Dialog.Header>
      <Dialog.Title>Duplicate Table</Dialog.Title>
      <Dialog.Description>
        Create a copy of <span class="font-mono text-foreground">"{schema}"."{tableName}"</span>
      </Dialog.Description>
    </Dialog.Header>

    <div class="space-y-4 py-2">
      <div class="space-y-1.5">
        <label for="dup-name" class="text-sm font-medium">New table name</label>
        <Input id="dup-name" bind:value={newName} placeholder="table_name" />
      </div>

      <div class="space-y-1.5">
        <span class="text-sm font-medium">Mode</span>
        <RadioGroup.Root bind:value={mode}>
          <div class="flex items-center gap-2">
            <RadioGroup.Item value="structure" id="dup-structure" />
            <label for="dup-structure" class="text-sm">Structure only</label>
          </div>
          <div class="flex items-center gap-2">
            <RadioGroup.Item value="data" id="dup-data" />
            <label for="dup-data" class="text-sm">Structure + Data</label>
          </div>
        </RadioGroup.Root>
      </div>
    </div>

    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (open = false)} disabled={loading}>
        Cancel
      </Button>
      <Button size="sm" onclick={handleDuplicate} disabled={loading || !newName.trim()}>
        {loading ? 'Duplicating...' : 'Duplicate'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
