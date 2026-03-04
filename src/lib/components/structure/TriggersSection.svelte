<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { StructureTab } from '$lib/types';
  import { Button } from '$lib/components/ui/button';
  import { Plus, Trash2, Power, PowerOff } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import ConfirmDialog from '$lib/components/ui/confirm-dialog/ConfirmDialog.svelte';
  import DdlPreview from './DdlPreview.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { generateCreateTrigger, generateDropTrigger, generateToggleTrigger } from '$lib/utils/ddl';

  let { tab }: { tab: StructureTab } = $props();

  const app = getAppState();

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
      ? generateCreateTrigger(tab.schema, tab.table, {
          name: addName,
          timing: addTiming,
          event: addEvent,
          forEach: addForEach,
          functionSchema: addFuncSchema,
          functionName: addFuncName,
          condition: addCondition || undefined,
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
    const sql = generateToggleTrigger(tab.schema, tab.table, triggerName, enable);
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
    const sql = generateDropTrigger(tab.schema, tab.table, dropName);
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
    <Button variant="outline" size="sm" class="h-7 text-xs" onclick={() => (addOpen = true)}>
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
        <input id="trig-name" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addName} />
      </div>
      <div class="grid grid-cols-3 gap-2">
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="trig-timing">Timing</label>
          <select id="trig-timing" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addTiming}>
            <option>BEFORE</option>
            <option>AFTER</option>
            <option>INSTEAD OF</option>
          </select>
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="trig-event">Event</label>
          <select id="trig-event" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addEvent}>
            <option>INSERT</option>
            <option>UPDATE</option>
            <option>DELETE</option>
            <option>TRUNCATE</option>
            <option>INSERT OR UPDATE</option>
            <option>INSERT OR DELETE</option>
            <option>UPDATE OR DELETE</option>
            <option>INSERT OR UPDATE OR DELETE</option>
          </select>
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="trig-foreach">For Each</label>
          <select id="trig-foreach" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addForEach}>
            <option>ROW</option>
            <option>STATEMENT</option>
          </select>
        </div>
      </div>
      <div class="grid grid-cols-2 gap-2">
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="trig-func-schema">Function Schema</label>
          <input id="trig-func-schema" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addFuncSchema} />
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground" for="trig-func-name">Function Name</label>
          <input id="trig-func-name" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addFuncName} />
        </div>
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="trig-condition">WHEN Condition (optional)</label>
        <input id="trig-condition" class="w-full mt-1 px-2 py-1.5 bg-card border border-border rounded text-sm text-foreground" bind:value={addCondition} placeholder="OLD.status IS DISTINCT FROM NEW.status" />
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
