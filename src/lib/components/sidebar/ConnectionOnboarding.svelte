<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { ConnectionInput, EngineType } from '$lib/types';
  import { Input } from '$lib/components/ui/input';
  import * as Select from '$lib/components/ui/select';
  import { Eye, EyeOff, CheckCircle, XCircle, Loader2, Database } from '@lucide/svelte';
    import { invoke } from '@tauri-apps/api/core';

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
    options: {},
  });

  let connectionUrl = $state('');
  let urlError = $state<string | null>(null);
  let showPassword = $state(false);
  let testing = $state(false);
  let testResult = $state<'success' | 'fail' | null>(null);
  let saving = $state(false);
  let connectError = $state<string | null>(null);
  let connecting = $state(false);

  const canSave = $derived(form.name.trim() && form.host.trim() && form.username.trim());

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
    const ok = await app.testConnection(form);
    testResult = ok ? 'success' : 'fail';
    testing = false;
  }

  async function handleConnect() {
    connectError = null;
    saving = true;
    await app.saveConnection(form);
    await app.loadConnections();
    saving = false;
    const conn = app.savedConnections.find(c => c.name === form.name);
    if (conn) {
      connecting = true;
      const error = await app.connectToDatabase(conn.id);
      connecting = false;
      if (error) connectError = error;
    }
  }
</script>

<div class="flex items-center justify-center h-full">
  <div class="w-full max-w-[420px] px-6">
    <!-- Header -->
    <div class="flex items-center gap-3 mb-8">
      <Database class="h-4 w-4 text-text-dim/50" />
      <h1 class="text-[15px] font-semibold text-foreground">New Connection</h1>
    </div>

    <!-- Engine (above URL so it controls which fields show) -->
    {#if availableEngines.length > 0}
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

    <!-- URL input -->
    <div class="mb-5">
      <label for="onboarding-url" class="block text-[12px] text-muted-foreground mb-1.5 select-none">Connection URL</label>
      <Input
        id="onboarding-url"
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

    <!-- Form fields -->
    <div class="space-y-4">
      <!-- Name -->
      <div class="flex items-center gap-3">
        <label for="onb-name" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Name</label>
        <Input id="onb-name" bind:value={form.name} placeholder="My Database" class="flex-1" />
      </div>

      <!-- Host -->
      <div class="flex items-center gap-3">
        <label for="onb-host" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Host</label>
        <Input id="onb-host" bind:value={form.host} placeholder="localhost" class="flex-1" />
      </div>

      <!-- Port -->
      <div class="flex items-center gap-3">
        <label for="onb-port" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Port</label>
        <Input id="onb-port" type="number" bind:value={form.port} class="flex-1 [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none" />
      </div>

      <!-- Database -->
      <div class="flex items-center gap-3">
        <label for="onb-db" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Database</label>
        <Input id="onb-db" bind:value={form.database} placeholder="postgres" class="flex-1" />
      </div>

      <!-- User -->
      <div class="flex items-center gap-3">
        <label for="onb-user" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">User</label>
        <Input id="onb-user" bind:value={form.username} placeholder="postgres" class="flex-1" />
      </div>

      <!-- Password -->
      <div class="flex items-center gap-3">
        <label for="onb-pass" class="w-20 shrink-0 text-[12px] text-muted-foreground select-none">Password</label>
        <div class="relative flex-1">
          <Input
            id="onb-pass"
            type={showPassword ? 'text' : 'password'}
            bind:value={form.password}
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
        class="h-[30px] px-3 text-[12px] font-medium rounded-md text-text-dim/80 hover:text-foreground hover:bg-accent/10 transition-all duration-100 disabled:opacity-30 disabled:pointer-events-none"
        onclick={handleTest}
        disabled={testing || !form.host}
      >
        {#if testing}
          <Loader2 class="h-3 w-3 mr-1.5 animate-spin inline" />
        {/if}
        Test
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
