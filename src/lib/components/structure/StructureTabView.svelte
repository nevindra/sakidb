<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { StructureTab, StructureSection } from '$lib/types';
  import { Loader2, RefreshCw, Pencil } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';
  import InputDialog from '$lib/components/ui/input-dialog/InputDialog.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import ColumnsSection from './ColumnsSection.svelte';
  import IndexesSection from './IndexesSection.svelte';
  import RelationsSection from './RelationsSection.svelte';
  import TriggersSection from './TriggersSection.svelte';
  import PartitionsSection from './PartitionsSection.svelte';
  import ProfilingSection from './ProfilingSection.svelte';

  let { tab }: { tab: StructureTab } = $props();

  const app = getAppState();

  const sections: { key: StructureSection; label: string }[] = [
    { key: 'columns', label: 'Columns' },
    { key: 'indexes', label: 'Indexes' },
    { key: 'relations', label: 'Relations' },
    { key: 'triggers', label: 'Triggers' },
    { key: 'partitions', label: 'Partitions' },
    { key: 'profiling', label: 'Profiling' },
  ];

  function setSection(section: StructureSection) {
    tab.activeSection = section;
  }

  function refresh() {
    app.loadStructureTab(tab.id);
  }

  let renameOpen = $state(false);
  let renameLoading = $state(false);

  async function handleRename(newName: string) {
    renameLoading = true;
    try {
      await invoke('execute_batch', {
        activeConnectionId: tab.runtimeConnectionId,
        sql: `ALTER TABLE "${tab.schema}"."${tab.table}" RENAME TO "${newName}";`,
      });
      tab.table = newName;
    } catch {
      // Error surfaced via app.error
    } finally {
      renameLoading = false;
    }
  }
</script>

<div class="flex flex-col h-full bg-background min-w-0">
  <!-- Header -->
  <div class="flex items-center justify-between px-4 py-2 border-b border-border">
    <div class="flex items-center gap-1.5 group/header">
      <span class="text-sm font-medium text-foreground">
        "{tab.schema}"."{tab.table}"
      </span>
      <Button
        variant="ghost"
        size="icon-sm"
        class="h-6 w-6 opacity-0 group-hover/header:opacity-100 transition-opacity"
        onclick={() => (renameOpen = true)}
      >
        <Pencil class="h-3 w-3 text-muted-foreground" />
      </Button>
    </div>
    <Button
      variant="ghost"
      size="icon-sm"
      class="h-7 w-7"
      onclick={refresh}
      disabled={tab.isLoading}
    >
      <RefreshCw class="h-3.5 w-3.5 {tab.isLoading ? 'animate-spin' : ''}" />
    </Button>
  </div>

  <!-- Section tabs -->
  <div class="flex items-center border-b border-border px-2 gap-0.5">
    {#each sections as section}
      <button
        class="px-3 py-1.5 text-xs font-medium transition-colors relative"
        class:text-foreground={tab.activeSection === section.key}
        class:text-muted-foreground={tab.activeSection !== section.key}
        onclick={() => setSection(section.key)}
      >
        {section.label}
        {#if tab.activeSection === section.key}
          <span class="absolute bottom-0 left-0 right-0 h-0.5 bg-primary"></span>
        {/if}
      </button>
    {/each}
  </div>

  <!-- Content -->
  {#if tab.isLoading}
    <div class="flex-1 flex items-center justify-center">
      <Loader2 class="h-5 w-5 animate-spin text-muted-foreground" />
    </div>
  {:else}
    <div class="flex-1 overflow-auto">
      {#if tab.activeSection === 'columns'}
        <ColumnsSection {tab} />
      {:else if tab.activeSection === 'indexes'}
        <IndexesSection {tab} />
      {:else if tab.activeSection === 'relations'}
        <RelationsSection {tab} />
      {:else if tab.activeSection === 'triggers'}
        <TriggersSection {tab} />
      {:else if tab.activeSection === 'partitions'}
        <PartitionsSection {tab} />
      {:else if tab.activeSection === 'profiling'}
        <ProfilingSection {tab} />
      {/if}
    </div>
  {/if}
</div>

<InputDialog
  bind:open={renameOpen}
  title="Rename Table"
  description={`Rename "${tab.schema}"."${tab.table}" to a new name.`}
  label="New name"
  placeholder={tab.table}
  initialValue={tab.table}
  confirmLabel="Rename"
  loading={renameLoading}
  onconfirm={handleRename}
/>
