<script lang="ts">
  import { save } from '@tauri-apps/plugin-dialog';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';
  import { listen } from '@tauri-apps/api/event';
  import { getAppState } from '$lib/stores';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as RadioGroup from '$lib/components/ui/radio-group';
  import { Switch } from '$lib/components/ui/switch';
  import { Button } from '$lib/components/ui/button';
  import { Loader2, CircleCheck, CircleX } from '@lucide/svelte';

  let {
    open = $bindable(false),
    savedConnectionId,
    databaseName,
    schema,
    table,
    whereClause,
  }: {
    open?: boolean;
    savedConnectionId: string;
    databaseName: string;
    schema: string;
    table: string;
    whereClause?: string;
  } = $props();

  const app = getAppState();
  const capabilities = $derived(app.getCapabilities(savedConnectionId));

  interface ExportProgressEvent {
    rows_exported: number;
    total_rows_estimate: number | null;
    phase: string;
  }

  let format = $state<'csv' | 'sql'>('csv');
  let includeHeader = $state(true);
  let includeDdl = $state(true);
  let includeData = $state(true);
  // svelte-ignore state_referenced_locally
  let useFilters = $state(!!whereClause);
  let exporting = $state(false);
  let resultMessage = $state('');
  let resultError = $state(false);
  let exportedFilePath = $state<string | null>(null);
  let progress = $state<ExportProgressEvent | null>(null);

  const progressPercent = $derived(
    progress && progress.total_rows_estimate && progress.total_rows_estimate > 0
      ? Math.min(100, Math.round((progress.rows_exported / progress.total_rows_estimate) * 100))
      : null
  );

  async function handleExport() {
    const ext = format === 'csv' ? 'csv' : 'sql';
    const defaultName = `${schema}_${table}.${ext}`;

    const filePath = await save({
      defaultPath: defaultName,
      filters: [
        format === 'csv'
          ? { name: 'CSV', extensions: ['csv'] }
          : { name: 'SQL', extensions: ['sql'] },
      ],
    });

    if (!filePath) return;

    exporting = true;
    resultMessage = '';
    resultError = false;
    exportedFilePath = null;
    progress = null;

    const unlisten = await listen<ExportProgressEvent>('export-progress', (event) => {
      progress = event.payload;
    });

    try {
      let rowCount: number;
      if (format === 'csv') {
        rowCount = await app.exportTableCsv(
          savedConnectionId,
          databaseName,
          schema,
          table,
          filePath,
          useFilters ? whereClause : undefined,
          includeHeader,
        );
        resultMessage = `Exported ${rowCount.toLocaleString()} rows to ${filePath}`;
      } else {
        rowCount = await app.exportTableSql(
          savedConnectionId,
          databaseName,
          schema,
          table,
          filePath,
          includeDdl,
          includeData,
        );
        resultMessage = `Exported ${includeDdl ? 'DDL + ' : ''}${rowCount.toLocaleString()} rows to ${filePath}`;
      }
      exportedFilePath = filePath;
    } catch (e) {
      resultMessage = `${String(e)}`;
      resultError = true;
    } finally {
      exporting = false;
      unlisten();
    }
  }

  async function handleCancel() {
    await app.cancelExport(savedConnectionId, databaseName);
  }

  async function handleReveal() {
    if (exportedFilePath) await revealItemInDir(exportedFilePath);
  }

  // Reset state when dialog closes
  $effect(() => {
    if (!open) {
      progress = null;
      resultMessage = '';
      resultError = false;
      exportedFilePath = null;
    }
  });
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-sm">
    <Dialog.Header>
      <Dialog.Title>Export {schema ? `"${schema}"."${table}"` : `"${table}"`}</Dialog.Title>
    </Dialog.Header>

    <div class="space-y-4 py-2">
      {#if exporting}
        <!-- Progress display -->
        <div class="space-y-3">
          <div class="w-full bg-muted rounded-full h-2">
            {#if progressPercent !== null}
              <div
                class="bg-primary h-2 rounded-full transition-all duration-200"
                style:width="{progressPercent}%"
              ></div>
            {:else}
              <div class="bg-primary h-2 rounded-full w-1/3 animate-pulse"></div>
            {/if}
          </div>

          <div class="text-xs text-muted-foreground space-y-1">
            <div class="flex justify-between">
              <span>{(progress?.rows_exported ?? 0).toLocaleString()} rows exported</span>
              {#if progressPercent !== null}
                <span>{progressPercent}%</span>
              {/if}
            </div>
            {#if progress?.total_rows_estimate}
              <div class="text-[10px]">
                of ~{progress.total_rows_estimate.toLocaleString()} total
              </div>
            {/if}
          </div>
        </div>
      {:else if resultMessage}
        <!-- Completion state -->
        <div class="flex flex-col items-center gap-3 py-4">
          {#if resultError}
            <CircleX class="h-10 w-10 text-destructive" />
          {:else}
            <CircleCheck class="h-10 w-10 text-primary" />
          {/if}
          <p class="text-sm text-muted-foreground text-center">{resultMessage}</p>
        </div>
      {:else}
        <!-- Format selection -->
        <div>
          <span class="text-xs font-medium text-muted-foreground">Format</span>
          <RadioGroup.Root bind:value={format} class="flex gap-4 mt-1.5">
            <label class="flex items-center gap-2 text-sm cursor-pointer">
              <RadioGroup.Item value="csv" />
              CSV
            </label>
            {#if capabilities?.sql !== false}
              <label class="flex items-center gap-2 text-sm cursor-pointer">
                <RadioGroup.Item value="sql" />
                SQL
              </label>
            {/if}
          </RadioGroup.Root>
        </div>

        <!-- CSV options -->
        {#if format === 'csv'}
          <div class="space-y-3">
            <label class="flex items-center justify-between cursor-pointer">
              <span class="text-xs">Include header row</span>
              <Switch bind:checked={includeHeader} />
            </label>
            {#if whereClause}
              <label class="flex items-center justify-between cursor-pointer">
                <span class="text-xs">Apply current filters</span>
                <Switch bind:checked={useFilters} />
              </label>
            {/if}
          </div>
        {/if}

        <!-- SQL options -->
        {#if format === 'sql'}
          <div class="space-y-3">
            <label class="flex items-center justify-between cursor-pointer">
              <span class="text-xs">Include DDL (CREATE TABLE)</span>
              <Switch bind:checked={includeDdl} />
            </label>
            <label class="flex items-center justify-between cursor-pointer">
              <span class="text-xs">Include data (COPY)</span>
              <Switch bind:checked={includeData} />
            </label>
          </div>
        {/if}
      {/if}
    </div>

    <Dialog.Footer>
      {#if exporting}
        <Button variant="outline" size="sm" onclick={handleCancel}>Cancel</Button>
      {:else if resultMessage}
        {#if !resultError && exportedFilePath}
          <Button variant="outline" size="sm" onclick={handleReveal}>Reveal in Folder</Button>
        {/if}
        <Button variant="outline" size="sm" onclick={() => (open = false)}>Close</Button>
      {:else}
        <Button variant="outline" size="sm" onclick={() => (open = false)}>Close</Button>
        <Button size="sm" onclick={handleExport} disabled={format === 'sql' && !includeDdl && !includeData}>
          Export
        </Button>
      {/if}
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
