<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { StructureTab, SchemaInfo, FunctionInfo } from '$lib/types';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Plus, Trash2, Power, PowerOff } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Select from '$lib/components/ui/select';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import DdlPreview from './DdlPreview.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { getDialect } from '$lib/dialects';
  import type { EngineType } from '$lib/types';

  let { tab }: { tab: StructureTab } = $props();

  const app = getAppState();
  const dialect = $derived((() => { const e = app.getSavedConnection(tab.savedConnectionId)?.engine; return e ? getDialect(e as EngineType) : null; })());

  const timingOptions = ['BEFORE', 'AFTER', 'INSTEAD OF'];
  const eventOptions = ['INSERT', 'UPDATE', 'DELETE', 'TRUNCATE', 'INSERT OR UPDATE', 'INSERT OR DELETE', 'UPDATE OR DELETE', 'INSERT OR UPDATE OR DELETE'];
  const forEachOptions = ['ROW', 'STATEMENT'];

  // ── Schema / function lookups ──
  let funcSchemas: string[] = $state([]);
  let funcNames: string[] = $state([]);

  function loadFuncSchemas() {
    const schemas = app.getSchemas(tab.savedConnectionId, tab.databaseName);
    funcSchemas = schemas.map((s: SchemaInfo) => s.name);
  }

  async function loadFuncNames(schema: string) {
    funcNames = [];
    const funcs = await app.loadFunctions(tab.savedConnectionId, tab.databaseName, schema);
    funcNames = funcs.map((f: FunctionInfo) => f.name);
  }

  // ── Create trigger dialog ──
  let addOpen = $state(false);
  let addName = $state('');
  let addTiming = $state('BEFORE');
  let addEvent = $state('INSERT');
  let addForEach = $state('ROW');
  let addFuncSchema = $state('public');
  let addFuncName = $state('');
  let addCondition = $state('');
  let addLoading = $state(false);

  const addSql = $derived(
    addName && addFuncName
      ? (dialect?.createTrigger(tab.schema, tab.table, {
          name: addName,
          timing: addTiming,
          event: addEvent,
          forEach: addForEach,
          functionSchema: addFuncSchema,
          functionName: addFuncName,
          condition: addCondition || undefined,
        }) ?? '')
      : ''
  );

  function handleOpenDialog() {
    addOpen = true;
    loadFuncSchemas();
    loadFuncNames(addFuncSchema);
  }

  function handleFuncSchemaChange(schema: string) {
    addFuncSchema = schema;
    addFuncName = '';
    loadFuncNames(schema);
  }

  async function handleAdd() {
    if (!addSql) return;
    addLoading = true;
    try {
      await app.executeDdl(tab.runtimeConnectionId, addSql);
      addOpen = false;
      addName = '';
      addFuncName = '';
      addCondition = '';
      app.loadStructureTab(tab.id);
    } catch {
      // Error shown via toast
    } finally {
      addLoading = false;
    }
  }

  // ── Toggle trigger ──
  async function toggleTrigger(triggerName: string, enable: boolean) {
    const sql = dialect?.toggleTrigger(tab.schema, tab.table, triggerName, enable);
    if (!sql) return;
    try {
      await app.executeDdl(tab.runtimeConnectionId, sql);
      app.loadStructureTab(tab.id);
    } catch {
      // Error shown via toast
    }
  }

  // ── Drop trigger ──
  let dropOpen = $state(false);
  let dropName = $state('');

  function confirmDrop(name: string) {
    dropName = name;
    dropOpen = true;
  }

  async function handleDrop() {
    const sql = dialect!.dropTrigger(tab.schema, tab.table, dropName);
    try {
      await app.executeDdl(tab.runtimeConnectionId, sql);
      app.loadStructureTab(tab.id);
    } catch {
      // Error shown via toast
    }
  }
</script>

<div class="p-3">
  <table class="w-full text-xs">
    <thead>
      <tr class="text-left text-muted-foreground border-b border-border">
        <th class="py-1.5 px-2 font-medium">Name</th>
        <th class="py-1.5 px-2 font-medium">Timing</th>
        <th class="py-1.5 px-2 font-medium">Event</th>
        <th class="py-1.5 px-2 font-medium">For Each</th>
        <th class="py-1.5 px-2 font-medium">Function</th>
        <th class="py-1.5 px-2 font-medium">Status</th>
        <th class="py-1.5 px-2 font-medium w-16"></th>
      </tr>
    </thead>
    <tbody>
      {#each tab.triggers as trig (trig.name)}
        <tr class="border-b border-border/50 hover:bg-sidebar-accent/50 transition-colors">
          <td class="py-1.5 px-2 font-medium text-foreground">{trig.name}</td>
          <td class="py-1.5 px-2 text-muted-foreground">{trig.timing}</td>
          <td class="py-1.5 px-2 text-muted-foreground">{trig.event}</td>
          <td class="py-1.5 px-2 text-muted-foreground">{trig.for_each}</td>
          <td class="py-1.5 px-2 text-muted-foreground font-mono">{trig.function_schema}.{trig.function_name}</td>
          <td class="py-1.5 px-2">
            {#if trig.is_enabled}
              <Badge variant="outline" class="text-[10px] py-0 px-1.5 text-success border-success/30">ON</Badge>
            {:else}
              <Badge variant="outline" class="text-[10px] py-0 px-1.5 text-muted-foreground">OFF</Badge>
            {/if}
          </td>
          <td class="py-1.5 px-2">
            <div class="flex items-center gap-1">
              <button
                class="p-0.5 text-muted-foreground hover:text-foreground transition-colors"
                onclick={() => toggleTrigger(trig.name, !trig.is_enabled)}
                title={trig.is_enabled ? 'Disable trigger' : 'Enable trigger'}
              >
                {#if trig.is_enabled}
                  <PowerOff class="h-3 w-3" />
                {:else}
                  <Power class="h-3 w-3" />
                {/if}
              </button>
              <button
                class="p-0.5 text-muted-foreground hover:text-destructive transition-colors"
                onclick={() => confirmDrop(trig.name)}
              >
                <Trash2 class="h-3 w-3" />
              </button>
            </div>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>

  {#if tab.triggers.length === 0}
    <p class="text-xs text-muted-foreground py-4 text-center">No triggers</p>
  {/if}

  <div class="mt-3">
    <Button variant="outline" size="sm" class="h-7 text-xs" onclick={handleOpenDialog}>
      <Plus class="h-3 w-3 mr-1" />
      Create Trigger
    </Button>
  </div>
</div>

<!-- Create Trigger Dialog -->
<Dialog.Root bind:open={addOpen}>
  <Dialog.Content class="max-w-md">
    <Dialog.Header>
      <Dialog.Title>Create Trigger</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-3 py-2">
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="trig-name">Trigger Name</label>
        <Input id="trig-name" class="mt-1" bind:value={addName} />
      </div>
      <div class="grid grid-cols-3 gap-2">
        <div>
          <label class="text-xs font-medium text-muted-foreground">Timing</label>
          <div class="mt-1">
            <Select.Root type="single" bind:value={addTiming}>
              <Select.Trigger class="w-full">
                <span data-slot="select-value">{addTiming}</span>
              </Select.Trigger>
              <Select.Content>
                {#each timingOptions as t}
                  <Select.Item value={t} label={t} />
                {/each}
              </Select.Content>
            </Select.Root>
          </div>
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground">Event</label>
          <div class="mt-1">
            <Select.Root type="single" bind:value={addEvent}>
              <Select.Trigger class="w-full">
                <span data-slot="select-value">{addEvent}</span>
              </Select.Trigger>
              <Select.Content>
                {#each eventOptions as e}
                  <Select.Item value={e} label={e} />
                {/each}
              </Select.Content>
            </Select.Root>
          </div>
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground">For Each</label>
          <div class="mt-1">
            <Select.Root type="single" bind:value={addForEach}>
              <Select.Trigger class="w-full">
                <span data-slot="select-value">{addForEach}</span>
              </Select.Trigger>
              <Select.Content>
                {#each forEachOptions as f}
                  <Select.Item value={f} label={f} />
                {/each}
              </Select.Content>
            </Select.Root>
          </div>
        </div>
      </div>
      <div class="grid grid-cols-2 gap-2">
        <div>
          <label class="text-xs font-medium text-muted-foreground">Function Schema</label>
          <div class="mt-1">
            <Select.Root type="single" value={addFuncSchema} onValueChange={(v) => { if (v) handleFuncSchemaChange(v); }}>
              <Select.Trigger class="w-full">
                <span data-slot="select-value">{addFuncSchema}</span>
              </Select.Trigger>
              <Select.Content>
                {#each funcSchemas as s}
                  <Select.Item value={s} label={s} />
                {/each}
              </Select.Content>
            </Select.Root>
          </div>
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground">Function Name</label>
          <div class="mt-1">
            <Select.Root type="single" bind:value={addFuncName}>
              <Select.Trigger class="w-full">
                <span data-slot="select-value">{addFuncName || 'Select function...'}</span>
              </Select.Trigger>
              <Select.Content>
                {#each funcNames as f}
                  <Select.Item value={f} label={f} />
                {/each}
              </Select.Content>
            </Select.Root>
          </div>
        </div>
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="trig-condition">WHEN Condition (optional)</label>
        <Input id="trig-condition" class="mt-1" bind:value={addCondition} placeholder="OLD.status IS DISTINCT FROM NEW.status" />
      </div>
      <DdlPreview sql={addSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (addOpen = false)} disabled={addLoading}>Cancel</Button>
      <Button size="sm" onclick={handleAdd} disabled={!addName || !addFuncName || addLoading}>Execute</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Drop Trigger Confirm -->
<ConfirmDialog
  bind:open={dropOpen}
  title="Drop Trigger"
  description={`Are you sure you want to drop trigger "${dropName}"?`}
  confirmLabel="Drop"
  variant="destructive"
  onconfirm={handleDrop}
/>
