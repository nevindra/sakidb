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
    connectionId,
    databaseName,
    materialized = false,
    oncreated,
  }: {
    open?: boolean;
    schema: string;
    connectionId: string;
    databaseName: string;
    materialized?: boolean;
    oncreated?: () => void;
  } = $props();

  const app = getAppState();
  const engine = $derived(app.getSavedConnection(connectionId)?.engine as EngineType | undefined);
  const dialect = $derived(engine ? getDialect(engine) : null);

  let viewName = $state('');
  let sqlBody = $state('');
  let loading = $state(false);

  const createSql = $derived.by(() => {
    if (!dialect || !viewName || !sqlBody) return '';
    if (materialized) {
      return dialect.createMaterializedView(schema, viewName, sqlBody);
    }
    return dialect.createView(schema, viewName, sqlBody, false);
  });

  function resetForm() {
    viewName = '';
    sqlBody = '';
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
  <Dialog.Content class="max-w-lg">
    <Dialog.Header>
      <Dialog.Title>{materialized ? 'Create Materialized View' : 'Create View'}</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-3 py-2">
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="view-name">Name</label>
        <Input id="view-name" class="mt-1" bind:value={viewName} placeholder="view_name" />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="view-sql">SQL Query</label>
        <textarea
          id="view-sql"
          class="mt-1 w-full h-32 px-3 py-2 text-xs font-mono bg-transparent border border-border rounded-md resize-y focus:outline-none focus:ring-1 focus:ring-ring"
          bind:value={sqlBody}
          placeholder="SELECT * FROM table_name WHERE ..."
        ></textarea>
      </div>
      <DdlPreview sql={createSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (open = false)} disabled={loading}>Cancel</Button>
      <Button size="sm" onclick={handleCreate} disabled={!viewName || !sqlBody || loading}>Execute</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
