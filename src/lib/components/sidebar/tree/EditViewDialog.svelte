<script lang="ts">
  import { getAppState } from '$lib/stores';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import * as Dialog from '$lib/components/ui/dialog';
  import DdlPreview from '../../structure/DdlPreview.svelte';
  import { getDialect } from '$lib/dialects';
  import type { EngineType } from '$lib/types';

  let {
    open = $bindable(false),
    schema,
    viewName,
    connectionId,
    databaseName,
    materialized = false,
    onedited,
  }: {
    open?: boolean;
    schema: string;
    viewName: string;
    connectionId: string;
    databaseName: string;
    materialized?: boolean;
    onedited?: () => void;
  } = $props();

  const app = getAppState();
  const engine = $derived(app.getSavedConnection(connectionId)?.engine as EngineType | undefined);
  const dialect = $derived(engine ? getDialect(engine) : null);

  let name = $state('');
  let sqlBody = $state('');
  let loading = $state(false);

  const editSql = $derived(
    name && sqlBody && dialect
      ? materialized
        ? `${dialect.dropMaterializedView(schema, name, false)}\n${dialect.createMaterializedView(schema, name, sqlBody)}`
        : dialect.createView(schema, name, sqlBody, true)
      : ''
  );

  function populateForm() {
    name = viewName;
    sqlBody = '';
  }

  $effect(() => {
    if (open) {
      populateForm();
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
  <Dialog.Content class="max-w-lg">
    <Dialog.Header>
      <Dialog.Title>{materialized ? 'Edit Materialized View' : 'Edit View'}</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-3 py-2">
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="edit-view-name">View Name</label>
        <Input id="edit-view-name" class="mt-1" bind:value={name} placeholder="view_name" />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="edit-view-sql">SQL Query</label>
        <textarea
          id="edit-view-sql"
          class="mt-1 w-full h-32 px-3 py-2 text-xs font-mono bg-transparent border border-border rounded-md resize-y focus:outline-none focus:ring-1 focus:ring-ring"
          bind:value={sqlBody}
          placeholder="SELECT * FROM table_name WHERE ..."
        ></textarea>
      </div>
      <DdlPreview sql={editSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (open = false)} disabled={loading}>Cancel</Button>
      <Button size="sm" onclick={handleEdit} disabled={!name || !sqlBody || loading}>
        {loading ? 'Executing...' : 'Execute'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
