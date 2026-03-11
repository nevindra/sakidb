<script lang="ts">
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import { ContextMenuRenderer, connectionManagerMenuItems } from '$lib/context-menus';
  import * as Select from '$lib/components/ui/select';
  import { Input } from '$lib/components/ui/input';
  import { getAppState } from '$lib/stores';
  import type { ConnectionInput, EngineType, SavedConnection } from '$lib/types';
  import { invoke } from '@tauri-apps/api/core';
  import { CheckCircle, Database, Eye, EyeOff, FolderOpen, Loader2, Plus, Search, XCircle } from '@lucide/svelte';
  import { open } from '@tauri-apps/plugin-dialog';

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

  // ── Left panel state ──
  let searchQuery = $state('');
  let selectedConnectionId = $state<string | null>(null);
  let isNewMode = $state(false);

  // ── Right panel form state ──
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
  let connectError = $state<string | null>(null);

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

  // ── Derived state ──
  const filteredConnections = $derived(
    searchQuery
      ? app.savedConnections.filter(c =>
          c.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
          c.host.toLowerCase().includes(searchQuery.toLowerCase()) ||
          c.database.toLowerCase().includes(searchQuery.toLowerCase())
        )
      : app.savedConnections
  );

  const recentConnections = $derived(
    app.savedConnections
      .filter(c => c.last_connected_at != null)
      .sort((a, b) => {
        const aTime = new Date(a.last_connected_at!).getTime();
        const bTime = new Date(b.last_connected_at!).getTime();
        return bTime - aTime;
      })
  );

  const filteredRecent = $derived(
    searchQuery
      ? recentConnections.filter(c =>
          c.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
          c.host.toLowerCase().includes(searchQuery.toLowerCase()) ||
          c.database.toLowerCase().includes(searchQuery.toLowerCase())
        )
      : recentConnections
  );

  const isEditing = $derived(selectedConnectionId !== null && !isNewMode);
  const isFileBased = $derived(form.engine === 'sqlite' || form.engine === 'duckdb');
  const canSave = $derived(
    isFileBased
      ? !!(form.name.trim() && form.database.trim())
      : !!(form.name.trim() && form.host.trim() && form.username.trim())
  );
  const connecting = $derived(selectedConnectionId ? app.isConnecting(selectedConnectionId) : false);

  const formTitle = $derived.by(() => {
    if (isNewMode || (!selectedConnectionId && app.savedConnections.length === 0)) return 'New Connection';
    if (selectedConnectionId) {
      const conn = app.savedConnections.find(c => c.id === selectedConnectionId);
      return conn?.name ?? 'New Connection';
    }
    return 'New Connection';
  });

  // ── Auto-select first connection or new mode ──
  $effect(() => {
    if (app.savedConnections.length === 0 && !isNewMode) {
      handleNewConnection();
    } else if (app.savedConnections.length > 0 && !selectedConnectionId && !isNewMode) {
      selectConnection(app.savedConnections[0]);
    }
  });

  // ── Helpers ──
  function formatTime(dateStr: string): string {
    try {
      const date = new Date(dateStr);
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffMins = Math.floor(diffMs / 60000);
      if (diffMins < 1) return 'just now';
      if (diffMins < 60) return `${diffMins}m ago`;
      const diffHours = Math.floor(diffMins / 60);
      if (diffHours < 24) return `${diffHours}h ago`;
      const diffDays = Math.floor(diffHours / 24);
      if (diffDays < 7) return `${diffDays}d ago`;
      const diffWeeks = Math.floor(diffDays / 7);
      if (diffWeeks < 4) return `${diffWeeks}w ago`;
      return date.toLocaleDateString();
    } catch {
      return '';
    }
  }

  function resetForm() {
    form = {
      name: '',
      engine: 'postgres',
      host: 'localhost',
      port: 5432,
      database: 'postgres',
      username: 'postgres',
      password: '',
      ssl_mode: 'prefer',
    };
    connectionUrl = '';
    urlError = null;
    showPassword = false;
    testResult = null;
  }

  function selectConnection(conn: SavedConnection) {
    isNewMode = false;
    selectedConnectionId = conn.id;
    form = {
      name: conn.name,
      engine: conn.engine || 'postgres',
      host: conn.host,
      port: conn.port,
      database: conn.database,
      username: conn.username,
      password: '',
      ssl_mode: conn.ssl_mode,
    };
    showPassword = false;
    testResult = null;
    connectError = null;
  }

  function handleNewConnection() {
    isNewMode = true;
    selectedConnectionId = null;
    resetForm();
    connectError = null;
  }

  async function handleTest() {
    testing = true;
    testResult = null;
    const ok = await app.testConnection(form, selectedConnectionId ?? undefined);
    testResult = ok ? 'success' : 'fail';
    testing = false;
  }

  async function handleConnect() {
    connectError = null;
    let error: string | null = null;
    if (isNewMode) {
      saving = true;
      await app.saveConnection(form);
      saving = false;
      await app.loadConnections();
      const conn = app.savedConnections.find(c => c.name === form.name);
      if (conn) {
        error = await app.connectToDatabase(conn.id);
      }
    } else if (selectedConnectionId) {
      if (form.password) {
        saving = true;
        await app.updateConnection(selectedConnectionId, form);
        saving = false;
      }
      error = await app.connectToDatabase(selectedConnectionId);
    }
    if (error) connectError = error;
  }

  async function handleSave() {
    saving = true;
    if (isNewMode) {
      await app.saveConnection(form);
      await app.loadConnections();
      const conn = app.savedConnections.find(c => c.name === form.name);
      if (conn) {
        selectConnection(conn);
      }
    } else if (selectedConnectionId) {
      await app.updateConnection(selectedConnectionId, form);
    }
    saving = false;
  }

  async function handleDelete(id: string) {
    await app.deleteConnection(id);
    if (selectedConnectionId === id) {
      if (app.savedConnections.length > 0) {
        selectConnection(app.savedConnections[0]);
      } else {
        handleNewConnection();
      }
    }
  }

  function handleDuplicate(conn: SavedConnection) {
    isNewMode = true;
    selectedConnectionId = null;
    form = {
      name: `${conn.name} (copy)`,
      engine: conn.engine || 'postgres',
      host: conn.host,
      port: conn.port,
      database: conn.database,
      username: conn.username,
      password: '',
      ssl_mode: conn.ssl_mode,
    };
    testResult = null;
  }
</script>

<div class="flex h-full bg-background">
  <!-- Left Panel -->
  <div class="w-64 shrink-0 flex flex-col border-r border-border/[0.06] bg-background">
    <!-- Header -->
    <div class="px-4 pt-3.5 pb-2 flex items-center justify-between">
      <span class="text-[12px] font-medium text-text-dim/90">Connections</span>
      <button
        class="w-6 h-6 flex items-center justify-center rounded-md text-text-dim/60 hover:text-foreground hover:bg-accent/10 transition-all duration-100"
        onclick={handleNewConnection}
        aria-label="New connection"
      >
        <Plus class="h-3.5 w-3.5" />
      </button>
    </div>

    <!-- Search -->
    <div class="px-3 pb-2">
      <div class="relative">
        <Search class="absolute left-2.5 top-1/2 -translate-y-1/2 h-3 w-3 text-text-dim/50" />
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Filter..."
          class="w-full pl-7 pr-3 h-7 text-[12px] bg-transparent border border-transparent rounded-md text-foreground placeholder:text-text-dim/40 hover:bg-accent/10 focus:bg-accent/15 focus:border-border/15 focus:outline-none transition-all duration-100"
        />
      </div>
    </div>

    <!-- Connection lists -->
    <div class="flex-1 overflow-y-auto px-2 pt-1">
      <!-- Saved section -->
      {#if filteredConnections.length > 0}
        <div class="px-2 pt-1 pb-1.5">
          <span class="text-[10px] font-medium text-text-dim/60">Saved Connection</span>
        </div>

        {#each filteredConnections as conn (conn.id)}
          {@const isSelected = selectedConnectionId === conn.id && !isNewMode}
          <ContextMenu.Root>
            <ContextMenu.Trigger class="block w-full">
              <button
                class="w-full text-left px-2.5 py-[6px] rounded-md flex items-center gap-2.5 transition-all duration-100 group mb-px {isSelected ? 'bg-accent/40' : 'hover:bg-accent/15'}"
                onclick={() => selectConnection(conn)}
              >
                <Database class="h-3.5 w-3.5 shrink-0 transition-colors duration-100 {isSelected ? 'text-foreground/70' : 'text-text-dim/50 group-hover:text-text-dim/70'}" />

                <div class="flex-1 min-w-0">
                  <span class="text-[13px] truncate block transition-colors duration-100 {isSelected ? 'text-foreground' : 'text-muted-foreground group-hover:text-foreground'}">{conn.name}</span>
                </div>

                <span class="text-[10px] text-text-dim/50 shrink-0 font-mono opacity-0 group-hover:opacity-100 transition-opacity duration-100 {isSelected ? '!opacity-100' : ''}">{conn.port}</span>
              </button>
            </ContextMenu.Trigger>
            <ContextMenuRenderer items={connectionManagerMenuItems()} ctx={{}} onaction={(id) => {
              switch (id) {
                case 'edit': return selectConnection(conn);
                case 'duplicate': return handleDuplicate(conn);
                case 'delete': return handleDelete(conn.id);
              }
            }} />
          </ContextMenu.Root>
        {/each}
      {/if}

      <!-- Recent section -->
      {#if filteredRecent.length > 0}
        <div class="px-2 pt-4 pb-1.5">
          <span class="text-[10px] font-medium text-text-dim/60">Recent Connections</span>
        </div>

        {#each filteredRecent as conn (conn.id + '-recent')}
          {@const isSelected = selectedConnectionId === conn.id && !isNewMode}
          <button
            class="w-full text-left px-2.5 py-[6px] rounded-md flex items-center gap-2.5 transition-all duration-100 group mb-px {isSelected ? 'bg-accent/40' : 'hover:bg-accent/15'}"
            onclick={() => selectConnection(conn)}
          >
            <Database class="h-3.5 w-3.5 shrink-0 transition-colors duration-100 {isSelected ? 'text-foreground/70' : 'text-text-dim/50 group-hover:text-text-dim/70'}" />
            <div class="flex-1 min-w-0">
              <span class="text-[13px] truncate block transition-colors duration-100 {isSelected ? 'text-foreground' : 'text-muted-foreground group-hover:text-foreground'}">{conn.name}</span>
            </div>
            <span class="text-[10px] text-text-dim/50 shrink-0 tabular-nums">
              {formatTime(conn.last_connected_at!)}
            </span>
          </button>
        {/each}
      {/if}

      <!-- Empty state -->
      {#if app.savedConnections.length === 0}
        <div class="px-3 py-16 text-center">
          <Database class="h-5 w-5 mx-auto text-text-dim/40 mb-2.5" />
          <p class="text-[12px] text-text-dim/60">No connections yet</p>
        </div>
      {/if}
    </div>
  </div>

  <!-- Right Panel -->
  <div class="flex-1 flex items-start justify-center overflow-y-auto py-10 px-8">
    <div class="max-w-[420px] w-full">
      <!-- Header -->
      <div class="mb-7">
        <h2 class="text-[15px] font-semibold text-foreground">{formTitle}</h2>
        {#if isEditing}
          <p class="text-[11px] text-text-dim/70 mt-1 font-mono">{isFileBased ? form.database || 'No file selected' : `${form.host}:${form.port}/${form.database}`}</p>
        {/if}
      </div>

      <!-- Engine (above URL so it controls which fields show) -->
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

      <!-- URL paste field (new mode only, not for file-based engines) -->
      {#if !isFileBased && (isNewMode || (!selectedConnectionId && app.savedConnections.length === 0))}
        <div class="mb-5">
          <label for="conn-mgr-url" class="block text-[12px] text-muted-foreground mb-1.5 select-none">Connection URL</label>
          <Input
            id="conn-mgr-url"
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
          <label for="cm-name" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Name</label>
          <Input id="cm-name" bind:value={form.name} placeholder="My Database" class="flex-1" />
        </div>

        <!-- Host (hidden for file-based engines) -->
        {#if form.engine !== 'sqlite' && form.engine !== 'duckdb'}
          <div class="flex items-center gap-3">
            <label for="cm-host" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Host</label>
            <Input id="cm-host" bind:value={form.host} placeholder="localhost" class="flex-1" />
          </div>

          <!-- Port -->
          <div class="flex items-center gap-3">
            <label for="cm-port" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Port</label>
            <Input id="cm-port" type="number" bind:value={form.port} class="flex-1 [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none" />
          </div>
        {/if}

        <!-- Database / File path (hidden for Redis) -->
        {#if form.engine !== 'redis'}
          <div class="flex items-center gap-3">
            <label for="cm-db" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">{isFileBased ? 'File' : 'Database'}</label>
            <div class="flex-1 flex gap-1.5">
              <Input id="cm-db" bind:value={form.database} placeholder={isFileBased ? '/path/to/database.db' : 'postgres'} class="flex-1" />
              {#if isFileBased}
                <button
                  class="h-9 w-9 shrink-0 flex items-center justify-center rounded-md border border-border/40 text-muted-foreground hover:text-foreground hover:bg-accent/10 transition-all duration-100"
                  aria-label="Browse for database file"
                  onclick={async () => {
                    const path = await open({
                      multiple: false,
                      filters: [{ name: 'SQLite Database', extensions: ['db', 'sqlite', 'sqlite3', 'db3'] }],
                    });
                    if (path) {
                      form.database = path;
                      if (!form.name) {
                        const fileName = path.split('/').pop()?.split('\\').pop() ?? path;
                        form.name = fileName;
                      }
                      testResult = null;
                    }
                  }}
                >
                  <FolderOpen class="h-3.5 w-3.5" />
                </button>
              {/if}
            </div>
          </div>
        {/if}

        <!-- User (hidden for file-based engines and Redis) -->
        {#if form.engine !== 'sqlite' && form.engine !== 'duckdb' && form.engine !== 'redis'}
          <div class="flex items-center gap-3">
            <label for="cm-user" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">User</label>
            <Input id="cm-user" bind:value={form.username} placeholder="postgres" class="flex-1" />
          </div>
        {/if}

        <!-- Password (hidden for file-based engines) -->
        {#if form.engine !== 'sqlite' && form.engine !== 'duckdb'}
          <div class="flex items-center gap-3">
            <label for="cm-pass" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Password</label>
            <div class="relative flex-1">
              <Input
                id="cm-pass"
                type={showPassword ? 'text' : 'password'}
                bind:value={form.password}
                placeholder={isEditing ? 'Unchanged' : ''}
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
        {/if}

        <!-- SSL (only for engines that support it) -->
        {#if form.engine === 'postgres' || form.engine === 'clickhouse' || form.engine === 'mongodb'}
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

      <!-- Connect error -->
      {#if connectError}
        <div class="flex items-start gap-2 text-destructive text-[12px] mt-5">
          <XCircle class="h-3.5 w-3.5 shrink-0 mt-0.5" />
          <span>{connectError}</span>
        </div>
      {/if}

      <!-- Actions -->
      <div class="flex items-center gap-2 mt-8">
        <button
          class="h-[30px] px-3 text-[12px] font-medium rounded-md text-text-dim/60 hover:text-foreground hover:bg-accent/10 transition-all duration-100 disabled:opacity-30 disabled:pointer-events-none"
          onclick={handleTest}
          disabled={testing || (isFileBased ? !form.database : !form.host)}
        >
          {#if testing}
            <Loader2 class="h-3 w-3 mr-1.5 animate-spin inline" />
          {/if}
          Test
        </button>

        <button
          class="h-[30px] px-3 text-[12px] font-medium rounded-md text-text-dim/60 hover:text-foreground hover:bg-accent/10 transition-all duration-100 disabled:opacity-30 disabled:pointer-events-none"
          onclick={handleSave}
          disabled={!canSave || saving}
        >
          {#if saving}
            <Loader2 class="h-3 w-3 mr-1.5 animate-spin inline" />
          {/if}
          Save
        </button>

        <div class="flex-1"></div>

        <button
          class="h-[30px] px-5 text-[12px] font-medium rounded-md bg-primary text-primary-foreground hover:brightness-110 active:brightness-95 transition-all duration-100 disabled:opacity-30 disabled:pointer-events-none"
          onclick={handleConnect}
          disabled={!canSave || saving || connecting}
        >
          {#if saving || connecting}
            <Loader2 class="h-3 w-3 mr-1.5 animate-spin inline" />
          {/if}
          {connecting ? 'Connecting…' : 'Connect'}
        </button>
      </div>
    </div>
  </div>
</div>
