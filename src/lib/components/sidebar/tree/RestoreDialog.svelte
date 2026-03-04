<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { listen } from '@tauri-apps/api/event';
  import { getAppState } from '$lib/stores';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import { Loader2 } from '@lucide/svelte';

  let {
    open: dialogOpen = $bindable(false),
    savedConnectionId,
    databaseName,
    schema,
    table,
  }: {
    open?: boolean;
    savedConnectionId: string;
    databaseName: string;
    schema?: string;
    table?: string;
  } = $props();

  const app = getAppState();

  interface RestoreProgress {
    bytes_read: number;
    total_bytes: number;
    statements_executed: number;
    errors_skipped: number;
    phase: string;
    elapsed_ms: number;
    error: string | null;
    error_messages: string[];
  }

  let filePath = $state<string | null>(null);
  let restoring = $state(false);
  let progress = $state<RestoreProgress | null>(null);
  let resultMessage = $state('');
  let resultError = $state(false);
  let continueOnError = $state(true);
  let errorMessages = $state<string[]>([]);
  let showErrors = $state(false);

  const targetLabel = $derived(
    table
      ? `${databaseName} / ${schema} / ${table}`
      : schema
        ? `${databaseName} / ${schema}`
        : databaseName
  );

  const progressPercent = $derived(
    progress && progress.total_bytes > 0
      ? Math.round((progress.bytes_read / progress.total_bytes) * 100)
      : 0
  );

  async function pickFile() {
    const selected = await open({
      filters: [{ name: 'SQL', extensions: ['sql'] }],
      multiple: false,
    });
    if (selected) {
      filePath = selected as string;
      resultMessage = '';
      resultError = false;
    }
  }

  async function handleRestore() {
    if (!filePath) return;

    restoring = true;
    progress = null;
    resultMessage = '';
    resultError = false;
    errorMessages = [];
    showErrors = false;

    let lastStmts = 0;
    let lastMs = 0;
    let lastErrors = 0;

    // Listen for progress events
    const unlisten = await listen<RestoreProgress>('restore-progress', (event) => {
      progress = event.payload;
      lastStmts = event.payload.statements_executed;
      lastMs = event.payload.elapsed_ms;
      lastErrors = event.payload.errors_skipped;
      if (event.payload.error_messages.length > 0) {
        errorMessages = event.payload.error_messages;
      }
    });

    try {
      await app.restoreFromSql(
        savedConnectionId,
        databaseName,
        filePath,
        schema,
        continueOnError,
      );
      let msg = `Restore complete: ${lastStmts.toLocaleString()} statements in ${formatTime(lastMs)}`;
      if (lastErrors > 0) {
        msg += ` (${lastErrors.toLocaleString()} errors skipped)`;
      }
      resultMessage = msg;
    } catch (e) {
      resultMessage = `Restore failed: ${String(e)}`;
      resultError = true;
    } finally {
      restoring = false;
      unlisten();
    }
  }

  async function handleCancel() {
    await app.cancelRestore();
  }

  function formatTime(ms: number): string {
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }

  // Reset state when dialog closes
  $effect(() => {
    if (!dialogOpen) {
      filePath = null;
      progress = null;
      resultMessage = '';
      resultError = false;
      errorMessages = [];
      showErrors = false;
    }
  });
</script>

<Dialog.Root bind:open={dialogOpen}>
  <Dialog.Content class="max-w-md">
    <Dialog.Header>
      <Dialog.Title>Restore from SQL</Dialog.Title>
      <Dialog.Description class="text-xs">
        Restoring to: {targetLabel}
      </Dialog.Description>
    </Dialog.Header>

    <div class="space-y-4 py-2">
      {#if restoring}
        <!-- Progress display -->
        <div class="space-y-3">
          <!-- Progress bar -->
          <div class="w-full bg-muted rounded-full h-2">
            <div
              class="bg-primary h-2 rounded-full transition-all duration-200"
              style:width="{progressPercent}%"
            ></div>
          </div>

          <!-- Stats -->
          <div class="text-xs text-muted-foreground space-y-1">
            <div class="flex justify-between">
              <span>{progress?.phase ?? 'Starting'}</span>
              <span>{progressPercent}%</span>
            </div>
            <div class="flex justify-between">
              <span>{(progress?.statements_executed ?? 0).toLocaleString()} statements</span>
              <span>{formatTime(progress?.elapsed_ms ?? 0)}</span>
            </div>
            {#if progress && progress.errors_skipped > 0}
              <div class="flex justify-between text-yellow-500">
                <span>{progress.errors_skipped.toLocaleString()} errors skipped</span>
              </div>
            {/if}
            {#if progress}
              <div class="text-[10px]">
                {formatBytes(progress.bytes_read)} / {formatBytes(progress.total_bytes)}
              </div>
            {/if}
          </div>
        </div>
      {:else if resultMessage}
        <!-- Result message -->
        <p class="text-xs bg-card border border-border rounded px-2 py-1.5"
           class:text-destructive={resultError}
           class:text-muted-foreground={!resultError}
        >
          {resultMessage}
        </p>

        <!-- Error list -->
        {#if errorMessages.length > 0}
          <button
            class="text-[11px] text-yellow-500 hover:underline cursor-pointer"
            onclick={() => (showErrors = !showErrors)}
          >
            {showErrors ? 'Hide' : 'Show'} {errorMessages.length} error{errorMessages.length > 1 ? 's' : ''}
          </button>

          {#if showErrors}
            <div class="max-h-40 overflow-y-auto rounded border border-border bg-card p-2 space-y-1">
              {#each errorMessages as msg}
                <p class="text-[10px] text-muted-foreground font-mono break-all">{msg}</p>
              {/each}
            </div>
          {/if}
        {/if}
      {:else}
        <!-- File selection -->
        <div class="space-y-2">
          <Button variant="outline" size="sm" class="w-full" onclick={pickFile}>
            {filePath ? filePath.split('/').pop() : 'Choose .sql file...'}
          </Button>
          {#if filePath}
            <p class="text-[10px] text-muted-foreground truncate">{filePath}</p>
          {/if}
        </div>

        <!-- Continue on error toggle -->
        <label class="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            bind:checked={continueOnError}
            class="h-3.5 w-3.5 rounded border-border accent-primary"
          />
          <span class="text-xs text-muted-foreground">Continue on error</span>
        </label>
      {/if}
    </div>

    <Dialog.Footer>
      {#if restoring}
        <Button variant="outline" size="sm" onclick={handleCancel}>Cancel</Button>
      {:else}
        <Button variant="outline" size="sm" onclick={() => (dialogOpen = false)}>Close</Button>
        {#if !resultMessage}
          <Button size="sm" onclick={handleRestore} disabled={!filePath}>
            Restore
          </Button>
        {/if}
      {/if}
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
