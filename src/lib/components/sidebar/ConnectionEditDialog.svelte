<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { ConnectionInput, EngineType } from '$lib/types';
  import { Input } from '$lib/components/ui/input';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Select from '$lib/components/ui/select';
  import { Eye, EyeOff, CheckCircle, XCircle, Loader2, FolderOpen } from '@lucide/svelte';
  import { open as tauriOpen } from '@tauri-apps/plugin-dialog';
  import { invoke } from '@tauri-apps/api/core';
  import OracleDriverDialog from './OracleDriverDialog.svelte';

  const app = getAppState();

  const ENGINE_LABELS: Record<EngineType, string> = {
    postgres: 'PostgreSQL',
    sqlite: 'SQLite',
    oracle: 'Oracle',
    redis: 'Redis',
    mongodb: 'MongoDB',
    duckdb: 'DuckDB',
    clickhouse: 'ClickHouse',
  };

  const ENGINE_DEFAULTS: Record<EngineType, { port: number; database: string; username: string }> = {
    postgres: { port: 5432, database: 'postgres', username: 'postgres' },
    sqlite: { port: 0, database: '', username: '' },
    oracle: { port: 1521, database: 'ORCL', username: '' },
    redis: { port: 6379, database: '', username: '' },
    mongodb: { port: 27017, database: '', username: '' },
    duckdb: { port: 0, database: '', username: '' },
    clickhouse: { port: 9000, database: 'default', username: 'default' },
  };

  let availableEngines = $state<EngineType[]>([]);
  invoke<EngineType[]>('available_engines').then(e => availableEngines = e);

  let form = $state<ConnectionInput>({
    name: '',
    engine: 'postgres',
    host: 'localhost',
    port: 5432,
    database: 'postgres',
    username: 'postgres',
    password: '',
    ssl_mode: 'prefer',
  });

  let connectionUrl = $state('');
  let urlError = $state<string | null>(null);
  let showPassword = $state(false);
  let testing = $state(false);
  let testResult = $state<'success' | 'fail' | null>(null);
  let saving = $state(false);
  let confirmDelete = $state(false);

  const isOpen = $derived(app.editDialogConnectionId !== null);
  const connection = $derived(
    app.editDialogConnectionId
      ? app.savedConnections.find(c => c.id === app.editDialogConnectionId)
      : null
  );

  $effect(() => {
    if (connection) {
      form = {
        name: connection.name,
        engine: connection.engine || 'postgres',
        host: connection.host,
        port: connection.port,
        database: connection.database,
        username: connection.username,
        password: '',
        ssl_mode: connection.ssl_mode,
      };
      connectionUrl = '';
      urlError = null;
      showPassword = false;
      testResult = null;
      confirmDelete = false;
    }
  });

  const isFileBased = $derived(form.engine === 'sqlite' || form.engine === 'duckdb');
  const canSave = $derived(
    isFileBased
      ? !!(form.name.trim() && form.database.trim())
      : !!(form.name.trim() && form.host.trim() && form.username.trim())
  );

  function parseConnectionUrl(raw: string) {
    const s = raw.trim();
    if (!s) { urlError = null; return; }
    if (!s.startsWith('postgresql://') && !s.startsWith('postgres://')) {
      urlError = 'URL must start with postgresql:// or postgres://';
      return;
    }
    try {
      const url = new URL(s);
      if (!url.hostname) { urlError = 'Missing host in URL'; return; }
      urlError = null;
      form.host = url.hostname;
      form.port = url.port ? Number(url.port) : 5432;
      form.database = url.pathname.replace(/^\//, '') || form.database;
      form.username = decodeURIComponent(url.username) || form.username;
      form.password = decodeURIComponent(url.password);
      const sslParam = url.searchParams.get('sslmode');
      if (sslParam && ['prefer', 'require', 'disable'].includes(sslParam)) {
        form.ssl_mode = sslParam;
      }
      if (!form.name) form.name = `${form.host}/${form.database}`;
      testResult = null;
    } catch {
      urlError = 'Invalid URL format';
    }
  }

  async function handleTest() {
    testing = true;
    testResult = null;
    const ok = await app.testConnection(form, app.editDialogConnectionId ?? undefined);
    testResult = ok ? 'success' : 'fail';
    testing = false;
  }

  async function handleSave() {
    if (!app.editDialogConnectionId) return;
    saving = true;
    await app.updateConnection(app.editDialogConnectionId, form);
    saving = false;
    app.closeEditDialog();
  }

  async function handleDelete() {
    if (!app.editDialogConnectionId) return;
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    await app.deleteConnection(app.editDialogConnectionId);
    app.closeEditDialog();
  }
</script>

<Dialog.Root
  open={isOpen}
  onOpenChange={(open) => { if (!open) app.closeEditDialog(); }}
>
  <Dialog.Content class="sm:max-w-[480px] gap-0 p-0 overflow-hidden">
    <!-- Header -->
    <div class="px-6 pt-5 pb-4">
      <Dialog.Title class="text-[15px] font-semibold text-foreground">{connection?.name ?? 'Edit Connection'}</Dialog.Title>
      <Dialog.Description class="text-[11px] text-muted-foreground font-mono mt-0.5">
        {isFileBased ? form.database || 'No file selected' : `${form.host}:${form.port}/${form.database}`}
      </Dialog.Description>
    </div>

    <!-- Form -->
    <div class="px-6 pb-5">
      <!-- Engine -->
      {#if availableEngines.length > 1}
        <div class="flex items-center gap-3 mb-5">
          <span class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Engine</span>
          <Select.Root type="single" value={form.engine} onValueChange={(v) => {
            if (v && v !== form.engine) {
              form.engine = v;
              const defaults = ENGINE_DEFAULTS[v as EngineType];
              if (defaults) {
                form.port = defaults.port;
                form.database = defaults.database;
                form.username = defaults.username;
              }
              testResult = null;
            }
          }}>
            <Select.Trigger class="flex-1 h-9 bg-transparent">
              <span class="text-foreground text-sm">{ENGINE_LABELS[form.engine as EngineType] ?? form.engine}</span>
            </Select.Trigger>
            <Select.Content>
              {#each availableEngines as engine (engine)}
                <Select.Item value={engine} label={ENGINE_LABELS[engine]} />
              {/each}
            </Select.Content>
          </Select.Root>
        </div>
      {/if}

      <!-- URL paste field (not for file-based engines) -->
      {#if !isFileBased}
      <div class="mb-5">
        <label for="ed-url" class="block text-[12px] text-muted-foreground mb-1.5 select-none">Connection URL</label>
        <Input
          id="ed-url"
          bind:value={connectionUrl}
          oninput={() => parseConnectionUrl(connectionUrl)}
          onpaste={(e) => {
            const text = e.clipboardData?.getData('text') ?? '';
            if (text.startsWith('postgresql://') || text.startsWith('postgres://')) {
              e.preventDefault();
              connectionUrl = text;
              parseConnectionUrl(text);
            }
          }}
          placeholder="postgresql://user:pass@host:5432/db"
          class="font-mono {urlError ? 'border-destructive' : ''}"
        />
        {#if urlError}
          <p class="text-[11px] text-destructive/70 mt-1">{urlError}</p>
        {/if}
      </div>
      {/if}

      <!-- Form fields -->
      <div class="space-y-4">
        <!-- Name -->
        <div class="flex items-center gap-3">
          <label for="ed-name" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Name</label>
          <Input id="ed-name" bind:value={form.name} placeholder="My Database" class="flex-1" />
        </div>

        <!-- Host (hidden for file-based engines) -->
        {#if !isFileBased}
          <div class="flex items-center gap-3">
            <label for="ed-host" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Host</label>
            <Input id="ed-host" bind:value={form.host} placeholder="localhost" class="flex-1" />
          </div>

          <!-- Port -->
          <div class="flex items-center gap-3">
            <label for="ed-port" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Port</label>
            <Input id="ed-port" type="number" bind:value={form.port} class="flex-1 [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none" />
          </div>
        {/if}

        <!-- Database / File path -->
        <div class="flex items-center gap-3">
          <label for="ed-db" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">{isFileBased ? 'File' : 'Database'}</label>
          <div class="flex-1 flex gap-1.5">
            <Input id="ed-db" bind:value={form.database} placeholder={isFileBased ? '/path/to/database.db' : 'postgres'} class="flex-1" />
            {#if isFileBased}
              <button
                class="h-9 w-9 shrink-0 flex items-center justify-center rounded-md border border-border/40 text-muted-foreground hover:text-foreground hover:bg-accent/10 transition-all duration-100"
                aria-label="Browse for database file"
                onclick={async () => {
                  const path = await tauriOpen({
                    multiple: false,
                    filters: [{ name: 'SQLite Database', extensions: ['db', 'sqlite', 'sqlite3', 'db3'] }],
                  });
                  if (path) {
                    form.database = path;
                    testResult = null;
                  }
                }}
              >
                <FolderOpen class="h-3.5 w-3.5" />
              </button>
            {/if}
          </div>
        </div>

        <!-- User (hidden for file-based engines) -->
        {#if !isFileBased}
          <div class="flex items-center gap-3">
            <label for="ed-user" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">User</label>
            <Input id="ed-user" bind:value={form.username} placeholder="postgres" class="flex-1" />
          </div>

          <!-- Password -->
          <div class="flex items-center gap-3">
            <label for="ed-pass" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Password</label>
            <div class="relative flex-1">
              <Input
                id="ed-pass"
                type={showPassword ? 'text' : 'password'}
                bind:value={form.password}
                placeholder="Unchanged"
                class="pr-8"
              />
              <button
                class="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground/60 hover:text-foreground transition-colors duration-100"
                aria-label="Toggle password visibility"
                onclick={() => showPassword = !showPassword}
              >
                {#if showPassword}
                  <EyeOff class="h-3.5 w-3.5" />
                {:else}
                  <Eye class="h-3.5 w-3.5" />
                {/if}
              </button>
            </div>
          </div>

          <!-- SSL -->
          {#if form.engine === 'postgres' || form.engine === 'oracle'}
            <div class="flex items-center gap-3">
              <span class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">SSL</span>
              <Select.Root type="single" value={form.ssl_mode} onValueChange={(v) => { if (v) form.ssl_mode = v; }}>
                <Select.Trigger class="flex-1 h-9 bg-transparent">
                  <span class="text-foreground text-sm">{form.ssl_mode === 'prefer' ? 'Prefer' : form.ssl_mode === 'require' ? 'Require' : 'Disable'}</span>
                </Select.Trigger>
                <Select.Content>
                  <Select.Item value="prefer" label="Prefer" />
                  <Select.Item value="require" label="Require" />
                  <Select.Item value="disable" label="Disable" />
                </Select.Content>
              </Select.Root>
            </div>
          {/if}
        {/if}
      </div>

      <!-- Test result -->
      {#if testResult === 'success'}
        <div class="flex items-center gap-2 text-success text-[12px] mt-5">
          <CheckCircle class="h-3.5 w-3.5" />
          Connection successful
        </div>
      {:else if testResult === 'fail'}
        <div class="flex items-center gap-2 text-destructive text-[12px] mt-5">
          <XCircle class="h-3.5 w-3.5" />
          Connection failed
        </div>
      {/if}
    </div>

    <!-- Footer -->
    <div class="px-6 py-3 border-t border-border/40 bg-card/50 flex items-center gap-2">
      <button
        class="h-[30px] px-3 text-[12px] font-medium rounded-md text-text-dim/80 hover:text-foreground hover:bg-accent/10 transition-all duration-100 disabled:opacity-30 disabled:pointer-events-none"
        onclick={handleTest}
        disabled={testing || (isFileBased ? !form.database : !form.host)}
      >
        {#if testing}
          <Loader2 class="h-3 w-3 mr-1.5 animate-spin inline" />
        {/if}
        Test
      </button>

      <button
        class="h-[30px] px-3 text-[12px] font-medium rounded-md text-destructive/80 hover:text-destructive hover:bg-destructive/10 transition-all duration-100"
        onclick={handleDelete}
      >
        {confirmDelete ? 'Confirm Delete' : 'Delete'}
      </button>

      <div class="flex-1"></div>

      <button
        class="h-[30px] px-5 text-[12px] font-medium rounded-md bg-primary text-primary-foreground hover:brightness-110 active:brightness-95 transition-all duration-100 disabled:opacity-30 disabled:pointer-events-none"
        onclick={handleSave}
        disabled={!canSave || saving}
      >
        {#if saving}
          <Loader2 class="h-3 w-3 animate-spin inline mr-1.5" />
        {/if}
        Save
      </button>
    </div>
  </Dialog.Content>
</Dialog.Root>
