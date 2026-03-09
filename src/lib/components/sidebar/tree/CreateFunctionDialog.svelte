<script lang="ts">
  import { getAppState } from '$lib/stores';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Select from '$lib/components/ui/select';
  import DdlPreview from '../../structure/DdlPreview.svelte';
  import { getDialect } from '$lib/dialects';
  import type { EngineType } from '$lib/types';

  let {
    open = $bindable(false),
    schema,
    connectionId,
    databaseName,
    oncreated,
  }: {
    open?: boolean;
    schema: string;
    connectionId: string;
    databaseName: string;
    oncreated?: () => void;
  } = $props();

  const app = getAppState();
  const engine = $derived(app.getSavedConnection(connectionId)?.engine as EngineType | undefined);
  const dialect = $derived(engine ? getDialect(engine) : null);

  const languageOptions = ['plpgsql', 'sql', 'plpython3u', 'plperl'];

  let name = $state('');
  let params = $state('');
  let returnType = $state('void');
  let language = $state('plpgsql');
  let body = $state('');
  let loading = $state(false);

  const createSql = $derived(
    name.trim()
      ? (dialect?.createFunction(schema, name.trim(), params, returnType, language, body, false) ?? '')
      : ''
  );

  function resetForm() {
    name = '';
    params = '';
    returnType = 'void';
    language = 'plpgsql';
    body = '';
  }

  $effect(() => {
    if (open) {
      resetForm();
    }
  });

  async function handleCreate() {
    if (!createSql) return;
    loading = true;
    try {
      const rid = app.getRuntimeConnectionId(connectionId, databaseName);
      if (!rid) return;
      await app.executeDdl(rid, createSql);
      open = false;
      resetForm();
      oncreated?.();
    } catch {
      // Error handled by store
    } finally {
      loading = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-lg">
    <Dialog.Header>
      <Dialog.Title>Create Function</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-3 py-2">
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="fn-name">Function Name</label>
        <Input id="fn-name" class="mt-1" bind:value={name} placeholder="my_function" />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="fn-params">Parameters</label>
        <Input id="fn-params" class="mt-1" bind:value={params} placeholder="p_name text, p_age integer" />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="fn-return">Return Type</label>
        <Input id="fn-return" class="mt-1" bind:value={returnType} placeholder="void" />
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground">Language</label>
        <div class="mt-1">
          <Select.Root type="single" bind:value={language}>
            <Select.Trigger class="w-full">
              <span data-slot="select-value">{language}</span>
            </Select.Trigger>
            <Select.Content>
              {#each languageOptions as lang}
                <Select.Item value={lang} label={lang} />
              {/each}
            </Select.Content>
          </Select.Root>
        </div>
      </div>
      <div>
        <label class="text-xs font-medium text-muted-foreground" for="fn-body">Function Body</label>
        <textarea
          id="fn-body"
          class="mt-1 w-full h-40 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring resize-y"
          bind:value={body}
          placeholder="BEGIN&#10;  -- function body here&#10;END;"
        ></textarea>
      </div>
      <DdlPreview sql={createSql} />
    </div>
    <Dialog.Footer>
      <Button variant="outline" size="sm" onclick={() => (open = false)} disabled={loading}>Cancel</Button>
      <Button size="sm" onclick={handleCreate} disabled={!name.trim() || loading}>
        {loading ? 'Creating...' : 'Execute'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
