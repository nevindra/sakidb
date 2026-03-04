<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { ConnectionInput } from '$lib/types';
  import { Input } from '$lib/components/ui/input';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Select from '$lib/components/ui/select';
  import { Eye, EyeOff, CheckCircle, XCircle, Loader2 } from '@lucide/svelte';

  const app = getAppState();

  let form = $state<ConnectionInput>({
    name: '',
    host: 'localhost',
    port: 5432,
    database: 'postgres',
    username: 'postgres',
    password: '',
    ssl_mode: 'prefer',
  });

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
        host: connection.host,
        port: connection.port,
        database: connection.database,
        username: connection.username,
        password: '',
        ssl_mode: connection.ssl_mode,
      };
      showPassword = false;
      testResult = null;
      confirmDelete = false;
    }
  });

  const canSave = $derived(form.name.trim() && form.host.trim() && form.username.trim());

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
      <Dialog.Title class="text-base font-semibold">{connection?.name ?? 'Edit Connection'}</Dialog.Title>
      <Dialog.Description class="text-xs text-text-dim font-mono mt-0.5">
        {form.host}:{form.port}/{form.database}
      </Dialog.Description>
    </div>

    <!-- Form -->
    <div class="px-6 pb-5 space-y-4">
      <!-- Name -->
      <div class="space-y-1.5">
        <label for="ed-name" class="text-[11px] font-medium text-text-dim uppercase tracking-wide">Name</label>
        <Input id="ed-name" bind:value={form.name} class="bg-background/50 border-border/50 focus:border-primary/50 transition-colors" />
      </div>

      <!-- Host + Port -->
      <div class="grid grid-cols-4 gap-3">
        <div class="col-span-3 space-y-1.5">
          <label for="ed-host" class="text-[11px] font-medium text-text-dim uppercase tracking-wide">Host</label>
          <Input id="ed-host" bind:value={form.host} class="bg-background/50 border-border/50 focus:border-primary/50 transition-colors" />
        </div>
        <div class="space-y-1.5">
          <label for="ed-port" class="text-[11px] font-medium text-text-dim uppercase tracking-wide">Port</label>
          <Input id="ed-port" type="number" bind:value={form.port} class="bg-background/50 border-border/50 focus:border-primary/50 transition-colors" />
        </div>
      </div>

      <!-- Database -->
      <div class="space-y-1.5">
        <label for="ed-db" class="text-[11px] font-medium text-text-dim uppercase tracking-wide">Database</label>
        <Input id="ed-db" bind:value={form.database} class="bg-background/50 border-border/50 focus:border-primary/50 transition-colors" />
      </div>

      <!-- Username + Password -->
      <div class="grid grid-cols-2 gap-3">
        <div class="space-y-1.5">
          <label for="ed-user" class="text-[11px] font-medium text-text-dim uppercase tracking-wide">User</label>
          <Input id="ed-user" bind:value={form.username} class="bg-background/50 border-border/50 focus:border-primary/50 transition-colors" />
        </div>
        <div class="space-y-1.5">
          <label for="ed-pass" class="text-[11px] font-medium text-text-dim uppercase tracking-wide">Password</label>
          <div class="relative">
            <Input
              id="ed-pass"
              type={showPassword ? 'text' : 'password'}
              bind:value={form.password}
              placeholder="Unchanged"
              class="pr-8 bg-background/50 border-border/50 focus:border-primary/50 transition-colors"
            />
            <button
              class="absolute right-2 top-1/2 -translate-y-1/2 text-text-dim hover:text-foreground transition-colors"
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
      </div>

      <!-- SSL Mode -->
      <div class="space-y-1.5">
        <span class="text-[11px] font-medium text-text-dim uppercase tracking-wide">SSL Mode</span>
        <Select.Root type="single" value={form.ssl_mode} onValueChange={(v) => { if (v) form.ssl_mode = v; }}>
          <Select.Trigger class="w-full h-9 bg-background/50 border-border/50 focus:border-primary/50 transition-colors">
            <span class="text-sm">{form.ssl_mode === 'prefer' ? 'Prefer' : form.ssl_mode === 'require' ? 'Require' : 'Disable'}</span>
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="prefer" label="Prefer" />
            <Select.Item value="require" label="Require" />
            <Select.Item value="disable" label="Disable" />
          </Select.Content>
        </Select.Root>
      </div>


      <!-- Test result -->
      {#if testResult === 'success'}
        <div class="flex items-center gap-2 text-success text-xs">
          <CheckCircle class="h-3.5 w-3.5" />
          Connection successful
        </div>
      {:else if testResult === 'fail'}
        <div class="flex items-center gap-2 text-destructive text-xs">
          <XCircle class="h-3.5 w-3.5" />
          Connection failed
        </div>
      {/if}
    </div>

    <!-- Footer -->
    <div class="px-6 py-3 border-t border-border/40 bg-card/50 flex items-center gap-2">
      <button
        class="h-8 px-3 text-xs font-medium rounded-lg border border-border/60 text-muted-foreground hover:text-foreground hover:border-border transition-all duration-150 disabled:opacity-40"
        onclick={handleTest}
        disabled={testing || !form.host}
      >
        {#if testing}
          <Loader2 class="h-3 w-3 mr-1.5 animate-spin inline" />
        {/if}
        Test
      </button>

      <button
        class="h-8 px-3 text-xs font-medium rounded-lg text-destructive/80 hover:text-destructive hover:bg-destructive/10 transition-all duration-150"
        onclick={handleDelete}
      >
        {confirmDelete ? 'Confirm Delete' : 'Delete'}
      </button>

      <div class="flex-1"></div>

      <button
        class="h-8 px-4 text-xs font-medium rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-all duration-150 disabled:opacity-40"
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
