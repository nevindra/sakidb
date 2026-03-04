<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { QueryHistoryEntry, SavedQuery } from '$lib/types';
  import { fuzzyMatch, type FuzzyResult } from '$lib/utils/fuzzy';
  import { ChevronRight, ChevronDown, Trash2 } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Tooltip from '$lib/components/ui/tooltip';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import QueryListItem from './QueryListItem.svelte';

  let { filterQuery = '' }: { filterQuery?: string } = $props();

  const app = getAppState();

  let savedExpanded = $state(true);
  let recentExpanded = $state(true);

  // ── Save dialog (promote recent → saved) ──
  let saveDialogOpen = $state(false);
  let saveDialogEntry = $state<QueryHistoryEntry | null>(null);
  let saveDialogName = $state('');

  // ── Edit dialog (edit existing saved query) ──
  let editDialogOpen = $state(false);
  let editDialogQuery = $state<SavedQuery | null>(null);
  let editDialogName = $state('');
  let editDialogSql = $state('');
  let confirmDelete = $state(false);

  // ── Filtered lists ──
  const savedWithMatches = $derived.by(() => {
    if (!filterQuery) return app.savedQueries.map(q => ({ query: q, match: null as FuzzyResult | null }));
    const results: { query: SavedQuery; match: FuzzyResult | null }[] = [];
    for (const q of app.savedQueries) {
      const nameMatch = fuzzyMatch(filterQuery, q.name);
      const sqlMatch = fuzzyMatch(filterQuery, q.sql);
      if (nameMatch || sqlMatch) {
        // Prefer name match for display, pick the higher score
        const best = nameMatch && sqlMatch
          ? (nameMatch.score >= sqlMatch.score ? nameMatch : null)
          : nameMatch;
        results.push({ query: q, match: best });
      }
    }
    return results.sort((a, b) => (b.match?.score ?? 0) - (a.match?.score ?? 0));
  });

  const filteredSaved = $derived(savedWithMatches.map(r => r.query));
  const savedMatchMap = $derived(new Map(savedWithMatches.filter(r => r.match).map(r => [r.query.id, r.match!])));

  const filteredRecent = $derived(
    filterQuery
      ? app.queryHistory.filter(q => fuzzyMatch(filterQuery, q.sql))
      : app.queryHistory
  );

  // ── Save dialog actions ──
  function openSaveDialog(entry: QueryHistoryEntry) {
    saveDialogEntry = entry;
    saveDialogName = '';
    saveDialogOpen = true;
  }

  async function handleSaveConfirm() {
    if (!saveDialogEntry || !saveDialogName.trim()) return;
    await app.saveFromHistory(saveDialogEntry.id, saveDialogName.trim());
    saveDialogOpen = false;
    saveDialogEntry = null;
  }

  // ── Edit dialog actions ──
  function openEditDialog(query: SavedQuery) {
    editDialogQuery = query;
    editDialogName = query.name;
    editDialogSql = query.sql;
    confirmDelete = false;
    editDialogOpen = true;
  }

  async function handleEditSave() {
    if (!editDialogQuery) return;
    await app.updateSavedQuery(editDialogQuery.id, editDialogName.trim() || undefined, editDialogSql || undefined);
    editDialogOpen = false;
    editDialogQuery = null;
  }

  async function handleEditDelete() {
    if (!editDialogQuery) return;
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    await app.deleteSavedQuery(editDialogQuery.id);
    editDialogOpen = false;
    editDialogQuery = null;
  }

  // ── Copy SQL ──
  function copySql(sql: string) {
    navigator.clipboard.writeText(sql);
  }

  // ── Open query in tab ──
  function openQuery(sql: string, connectionId: string | null, databaseName: string | null) {
    if (connectionId && databaseName) {
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (rid) {
        app.openQueryTab(connectionId, databaseName, sql);
        return;
      }
    }
    for (const [savedId, conn] of app.activeConnections) {
      const firstDb = conn.activeDatabases.keys().next();
      if (!firstDb.done) {
        app.openQueryTab(savedId, firstDb.value, sql);
        return;
      }
    }
  }
</script>

<div class="flex flex-col gap-0.5 py-1">
  <!-- Saved section -->
  <div class="flex items-center">
    <button
      class="flex items-center gap-1 flex-1 px-3 py-1 text-xs font-semibold uppercase text-muted-foreground tracking-wider hover:text-foreground transition-colors text-left"
      onclick={() => savedExpanded = !savedExpanded}
    >
      {#if savedExpanded}
        <ChevronDown class="h-3 w-3" />
      {:else}
        <ChevronRight class="h-3 w-3" />
      {/if}
      Saved
      {#if filteredSaved.length > 0}
        <span class="text-[10px] font-normal ml-1">({filteredSaved.length})</span>
      {/if}
    </button>
  </div>

  {#if savedExpanded}
    {#if filteredSaved.length === 0}
      <div class="px-3 py-2 text-[11px] text-muted-foreground">
        No saved queries
      </div>
    {:else}
      {#each filteredSaved as query (query.id)}
        <QueryListItem
          sql={query.sql}
          name={query.name}
          isSaved={true}
          nameMatch={savedMatchMap.get(query.id)}
          onOpen={() => openQuery(query.sql, query.connection_id, query.database_name)}
          onEdit={() => openEditDialog(query)}
          onDelete={() => app.deleteSavedQuery(query.id)}
          onCopySql={() => copySql(query.sql)}
        />
      {/each}
    {/if}
  {/if}

  <!-- Recent section -->
  <div class="flex items-center">
    <button
      class="flex items-center gap-1 flex-1 px-3 py-1 text-xs font-semibold uppercase text-muted-foreground tracking-wider hover:text-foreground transition-colors text-left"
      onclick={() => recentExpanded = !recentExpanded}
    >
      {#if recentExpanded}
        <ChevronDown class="h-3 w-3" />
      {:else}
        <ChevronRight class="h-3 w-3" />
      {/if}
      Recent
      {#if filteredRecent.length > 0}
        <span class="text-[10px] font-normal ml-1">({filteredRecent.length})</span>
      {/if}
    </button>
    {#if filteredRecent.length > 0}
      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              variant="ghost"
              size="icon-sm"
              class="h-5 w-5 mr-2 text-muted-foreground hover:text-destructive"
              onclick={() => app.clearHistory()}
            >
              <Trash2 class="h-3 w-3" />
            </Button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content>Clear history</Tooltip.Content>
      </Tooltip.Root>
    {/if}
  </div>

  {#if recentExpanded}
    {#if filteredRecent.length === 0}
      <div class="px-3 py-2 text-[11px] text-muted-foreground">
        No recent queries
      </div>
    {:else}
      {#each filteredRecent as entry (entry.id)}
        <QueryListItem
          sql={entry.sql}
          timestamp={entry.executed_at}
          isSaved={false}
          onOpen={() => openQuery(entry.sql, entry.connection_id, entry.database_name)}
          onSave={() => openSaveDialog(entry)}
          onCopySql={() => copySql(entry.sql)}
        />
      {/each}
    {/if}
  {/if}
</div>

<!-- Save query dialog (recent → saved) -->
<Dialog.Root
  bind:open={saveDialogOpen}
  onOpenChange={(open) => { if (!open) { saveDialogEntry = null; } }}
>
  <Dialog.Content class="sm:max-w-[480px] gap-0 p-0 overflow-hidden">
    <div class="px-6 pt-5 pb-4">
      <Dialog.Title class="text-base font-semibold">Save Query</Dialog.Title>
      <Dialog.Description class="text-xs text-muted-foreground mt-0.5">
        Give this query a name to save it for later.
      </Dialog.Description>
    </div>

    <div class="px-6 pb-5 space-y-4">
      <div class="space-y-1.5">
        <label for="sq-name" class="text-[11px] font-medium text-muted-foreground uppercase tracking-wide">Name</label>
        <Input
          id="sq-name"
          bind:value={saveDialogName}
          placeholder="e.g. Monthly report"
          class="bg-background/50 border-border/50 focus:border-primary/50 transition-colors"
        />
      </div>

      <div class="space-y-1.5">
        <span class="text-[11px] font-medium text-muted-foreground uppercase tracking-wide">SQL</span>
        <div class="px-3 py-2 rounded-md border border-border/50 bg-background/30 text-xs font-mono text-muted-foreground max-h-32 overflow-y-auto whitespace-pre-wrap break-all">
          {saveDialogEntry?.sql ?? ''}
        </div>
      </div>
    </div>

    <div class="px-6 py-3 border-t border-border/40 bg-card/50 flex items-center justify-end gap-2">
      <button
        class="h-8 px-3 text-xs font-medium rounded-lg border border-border/60 text-muted-foreground hover:text-foreground hover:border-border transition-all duration-150"
        onclick={() => saveDialogOpen = false}
      >
        Cancel
      </button>
      <button
        class="h-8 px-4 text-xs font-medium rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-all duration-150 disabled:opacity-40"
        onclick={handleSaveConfirm}
        disabled={!saveDialogName.trim()}
      >
        Save
      </button>
    </div>
  </Dialog.Content>
</Dialog.Root>

<!-- Edit saved query dialog -->
<Dialog.Root
  bind:open={editDialogOpen}
  onOpenChange={(open) => { if (!open) { editDialogQuery = null; confirmDelete = false; } }}
>
  <Dialog.Content class="sm:max-w-[520px] gap-0 p-0 overflow-hidden">
    <div class="px-6 pt-5 pb-4">
      <Dialog.Title class="text-base font-semibold">{editDialogQuery?.name ?? 'Edit Query'}</Dialog.Title>
      <Dialog.Description class="text-xs text-muted-foreground mt-0.5">
        Update the name or SQL content.
      </Dialog.Description>
    </div>

    <div class="px-6 pb-5 space-y-4">
      <div class="space-y-1.5">
        <label for="eq-name" class="text-[11px] font-medium text-muted-foreground uppercase tracking-wide">Name</label>
        <Input id="eq-name" bind:value={editDialogName} class="bg-background/50 border-border/50 focus:border-primary/50 transition-colors" />
      </div>

      <div class="space-y-1.5">
        <label for="eq-sql" class="text-[11px] font-medium text-muted-foreground uppercase tracking-wide">SQL</label>
        <textarea
          id="eq-sql"
          bind:value={editDialogSql}
          rows="8"
          class="w-full px-3 py-2 rounded-md border border-border/50 bg-background/50 text-sm font-mono text-foreground focus:outline-none focus:border-primary/50 transition-colors resize-y"
          spellcheck="false"
        ></textarea>
      </div>
    </div>

    <div class="px-6 py-3 border-t border-border/40 bg-card/50 flex items-center gap-2">
      <button
        class="h-8 px-3 text-xs font-medium rounded-lg text-destructive/80 hover:text-destructive hover:bg-destructive/10 transition-all duration-150"
        onclick={handleEditDelete}
      >
        {confirmDelete ? 'Confirm Delete' : 'Delete'}
      </button>

      <div class="flex-1"></div>

      <button
        class="h-8 px-3 text-xs font-medium rounded-lg border border-border/60 text-muted-foreground hover:text-foreground hover:border-border transition-all duration-150"
        onclick={() => editDialogOpen = false}
      >
        Cancel
      </button>

      <button
        class="h-8 px-4 text-xs font-medium rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-all duration-150 disabled:opacity-40"
        onclick={handleEditSave}
        disabled={!editDialogName.trim()}
      >
        Save
      </button>
    </div>
  </Dialog.Content>
</Dialog.Root>
