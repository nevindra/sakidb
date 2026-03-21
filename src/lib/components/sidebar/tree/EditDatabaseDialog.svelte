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
    databaseName,
    connectionId,
    isTemplate = false,
    onedited,
  }: {
    open?: boolean;
    databaseName: string;
    connectionId: string;
    isTemplate?: boolean;
    onedited?: () => void;
  } = $props();

  const app = getAppState();
  const engine = $derived(app.getSavedConnection(connectionId)?.engine as EngineType | undefined);
  const dialect = $derived(engine ? getDialect(engine) : null);

  let newName = $state('');
  let owner = $state('');
  let connectionLimit = $state('');
  let template = $state(false);
  let tablespace = $state('');
  let loading = $state(false);

  const isRenamed = $derived(newName.trim() !== '' && newName.trim() !== databaseName);

  const alterSql = $derived.by(() => {
    if (!dialect) return '';
    const quoted = dialect.quoteIdent(databaseName);
    const stmts: string[] = [];
    if (isRenamed) stmts.push(`ALTER DATABASE ${quoted} RENAME TO ${dialect.quoteIdent(newName.trim())};`);
    if (owner.trim()) stmts.push(`ALTER DATABASE ${quoted} OWNER TO ${dialect.quoteIdent(owner.trim())};`);
    if (connectionLimit.trim()) stmts.push(`ALTER DATABASE ${quoted} CONNECTION LIMIT ${Number(connectionLimit)};`);
    if (template !== isTemplate) stmts.push(`ALTER DATABASE ${quoted} IS_TEMPLATE ${template};`);
    if (tablespace.trim()) stmts.push(`ALTER DATABASE ${quoted} SET TABLESPACE ${dialect.quoteIdent(tablespace.trim())};`);
    return stmts.join('\n');
  });

  $effect(() => {
    if (open) {
      newName = databaseName;
      owner = '';
      connectionLimit = '';
      template = isTemplate;
      tablespace = '';
    }
  });

  async function handleEdit() {
    if (!alterSql) return;
    loading = true;
    try {
      // Rename uses a dedicated command (requires maintenance DB connection)
      if (isRenamed) {
        await app.renameDatabase(connectionId, databaseName, newName.trim());
      }
      // Other ALTER statements execute against the target database
      const nonRenameStmts = alterSql.split('\n').filter(s => !s.includes('RENAME TO'));
      const otherSql = nonRenameStmts.join('\n');
      if (otherSql) {
        const targetDb = isRenamed ? newName.trim() : databaseName;
        const rid = app.getRuntimeConnectionId(connectionId, targetDb);
        if (rid) {
          await app.executeDdl(rid, otherSql);
        }
      }
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
      <Dialog.Title>Edit Database</Dialog.Title>
      <Dialog.Description>
        Alter <span class="font-mono text-foreground">"{databaseName}"</span>. Only fill in fields you want to change.
      </Dialog.Description>
    </Dialog.Header>
    <div class="space-y-3 py-2">
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="db-name">Database Name</label>
        <Input id="db-name" class="mt-1" bind:value={newName} placeholder={databaseName} />
      </div>
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="db-owner">Owner</label>
          <Input id="db-owner" class="mt-1" bind:value={owner} placeholder="new_owner" />
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="db-conn-limit">Connection Limit</label>
          <Input id="db-conn-limit" class="mt-1" type="number" bind:value={connectionLimit} placeholder="-1 (unlimited)" />
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="db-tablespace">Tablespace</label>
          <Input id="db-tablespace" class="mt-1" bind:value={tablespace} placeholder="pg_default" />
        </div>
        <div class="flex items-end pb-1">
          <label class="flex items-center gap-2 text-xs font-medium text-muted-foreground">
            <Checkbox bind:checked={template} />
            Is Template
          </label>
        </div>
      </div>
      <DdlPreview sql={alterSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (open = false)} disabled={loading}>Cancel</Button>
      <Button size="sm" onclick={handleEdit} disabled={!alterSql || loading}>
        {loading ? 'Executing...' : 'Execute'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
