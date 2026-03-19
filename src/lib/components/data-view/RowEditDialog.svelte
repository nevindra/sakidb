<script lang="ts">
  import type { CellValue, ColumnDef, ColumnInfo } from '$lib/types';
  import { cellValueToEditText, parseInputToCellValue } from '$lib/sql-utils';
  import { getTypeCategory, getTypePlaceholder } from '$lib/type-utils';
  import { cellValueEquals } from '$lib/sql-utils';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';

  let {
    open = $bindable(false),
    mode,
    columns,
    columnInfos = [],
    values = null,
    schema = '',
    table = '',
    onconfirm,
  }: {
    open: boolean;
    mode: 'add' | 'edit';
    columns: ColumnDef[];
    columnInfos?: ColumnInfo[];
    values?: CellValue[] | null;
    schema?: string;
    table?: string;
    onconfirm: (values: CellValue[]) => void;
  } = $props();

  // Track field text values — initialized when dialog opens
  let fieldTexts = $state<string[]>([]);
  let jsonErrors = $state<boolean[]>([]);
  // Snapshot of original values for edit mode diff
  let originalTexts = $state<string[]>([]);

  const pkNames = $derived(new Set(
    columnInfos.filter(c => c.is_primary_key).map(c => c.name)
  ));

  const hasAutoDefault = $derived.by(() => {
    const autoDefaults = new Set<string>();
    for (const info of columnInfos) {
      if (info.default_value && (
        info.default_value.includes('nextval(') ||
        info.default_value.includes('gen_random_uuid') ||
        info.default_value.includes('uuid_generate')
      )) {
        autoDefaults.add(info.name);
      }
    }
    return autoDefaults;
  });

  // Reinitialize fields when dialog opens
  $effect(() => {
    if (open) {
      const texts = columns.map((col, i) => {
        if (mode === 'edit' && values) {
          return cellValueToEditText(values[i]);
        }
        return '';
      });
      fieldTexts = texts;
      jsonErrors = new Array(columns.length).fill(false);
      originalTexts = texts.slice();
    }
  });

  const modifiedCount = $derived.by(() => {
    if (mode !== 'edit') return 0;
    let count = 0;
    for (let i = 0; i < fieldTexts.length; i++) {
      if (fieldTexts[i] !== originalTexts[i]) count++;
    }
    return count;
  });

  function isFieldModified(idx: number): boolean {
    if (mode !== 'edit') return false;
    return fieldTexts[idx] !== originalTexts[idx];
  }

  function isPkField(col: ColumnDef): boolean {
    return pkNames.has(col.name);
  }

  function isAutoField(col: ColumnDef): boolean {
    return hasAutoDefault.has(col.name);
  }

  function isReadOnly(col: ColumnDef): boolean {
    const category = getTypeCategory(col.data_type);
    if (category === 'binary') return true;
    if (mode === 'edit' && isPkField(col)) return true;
    return false;
  }

  function handleJsonInput(idx: number) {
    const text = fieldTexts[idx].trim();
    if (!text || text.toLowerCase() === 'null') {
      jsonErrors[idx] = false;
      return;
    }
    try {
      JSON.parse(text);
      jsonErrors[idx] = false;
    } catch {
      jsonErrors[idx] = true;
    }
  }

  function formatJson(idx: number) {
    try {
      fieldTexts[idx] = JSON.stringify(JSON.parse(fieldTexts[idx]), null, 2);
      jsonErrors[idx] = false;
    } catch {
      jsonErrors[idx] = true;
    }
  }

  function minifyJson(idx: number) {
    try {
      fieldTexts[idx] = JSON.stringify(JSON.parse(fieldTexts[idx]));
      jsonErrors[idx] = false;
    } catch {
      jsonErrors[idx] = true;
    }
  }

  function setFieldNull(idx: number) {
    fieldTexts[idx] = '';
  }

  function handleConfirm() {
    // Validate JSON fields
    for (let i = 0; i < columns.length; i++) {
      const category = getTypeCategory(columns[i].data_type);
      if (category === 'json' && jsonErrors[i]) return;
    }

    const result: CellValue[] = columns.map((col, i) => {
      if (isReadOnly(col) && mode === 'edit' && values) {
        return values[i];
      }
      // For add mode, auto fields with empty input → Null (let DB handle default)
      if (mode === 'add' && isAutoField(col) && fieldTexts[i].trim() === '') {
        return 'Null';
      }
      const category = getTypeCategory(col.data_type);
      if (category === 'json') {
        const text = fieldTexts[i].trim();
        if (!text || text.toLowerCase() === 'null') return 'Null';
        return { Json: text };
      }
      if (category === 'boolean') {
        const text = fieldTexts[i].trim().toLowerCase();
        if (!text || text === 'null') return 'Null';
        return { Bool: text === 'true' || text === 't' || text === '1' || text === 'yes' };
      }
      return parseInputToCellValue(fieldTexts[i], col.data_type);
    });

    onconfirm(result);
    open = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleConfirm();
    }
  }
</script>

<svelte:window onkeydown={open ? handleKeydown : undefined} />

<Dialog.Root bind:open>
  <Dialog.Content showCloseButton={true} class="max-w-lg max-h-[80vh] flex flex-col gap-0 p-0">
    <Dialog.Header class="shrink-0 px-5 pt-5 pb-3 border-b border-border/60">
      <Dialog.Title class="text-sm font-semibold">
        {mode === 'add' ? 'Add New Row' : `Edit Row`}
      </Dialog.Title>
      <Dialog.Description class="text-xs text-muted-foreground">
        {schema ? `${schema}.${table}` : table} · {columns.length} columns
      </Dialog.Description>
    </Dialog.Header>

    <div class="flex-1 overflow-y-auto px-5 py-4 space-y-3.5">
      {#each columns as col, i}
        {@const category = getTypeCategory(col.data_type)}
        {@const placeholder = getTypePlaceholder(col.data_type)}
        {@const readonly = isReadOnly(col)}
        {@const isPk = isPkField(col)}
        {@const isAuto = isAutoField(col)}
        {@const modified = isFieldModified(i)}

        <div>
          <!-- Label row -->
          <div class="flex items-center justify-between mb-1">
            <div class="flex items-center gap-1.5">
              <label class="text-xs font-medium text-foreground" for="field-{i}">{col.name}</label>
              {#if isPk}
                <span class="text-[9px] px-1 py-0.5 rounded bg-primary/10 text-primary font-medium">PK</span>
              {/if}
            </div>
            <div class="flex items-center gap-1.5">
              <span class="text-[10px] text-muted-foreground">{col.data_type}</span>
              {#if !readonly && category !== 'boolean'}
                <button
                  class="text-[10px] px-1 py-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
                  onclick={() => setFieldNull(i)}
                  type="button"
                >NULL</button>
              {/if}
            </div>
          </div>

          <!-- Field input -->
          {#if readonly}
            <div class="w-full bg-muted/30 border border-border/40 rounded-md px-3 py-1.5 text-xs text-muted-foreground">
              {#if mode === 'add' && isAuto}
                <span class="italic">auto-generated</span>
              {:else if category === 'binary'}
                <span class="italic">binary (not editable)</span>
              {:else if values}
                {cellValueToEditText(values[i]) || 'NULL'}
              {/if}
            </div>

          {:else if category === 'boolean'}
            <select
              id="field-{i}"
              class="w-full bg-muted/20 border rounded-md px-3 py-1.5 text-xs text-foreground outline-none focus:border-primary/60 transition-colors
                {modified ? 'border-warning' : 'border-border/60'}"
              bind:value={fieldTexts[i]}
            >
              <option value="">NULL</option>
              <option value="true">true</option>
              <option value="false">false</option>
            </select>

          {:else if category === 'json'}
            <div>
              <div class="flex items-center gap-1 mb-1">
                <button
                  class="text-[10px] px-1.5 py-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
                  onclick={() => formatJson(i)}
                  type="button"
                >Format</button>
                <button
                  class="text-[10px] px-1.5 py-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
                  onclick={() => minifyJson(i)}
                  type="button"
                >Minify</button>
                {#if jsonErrors[i]}
                  <span class="text-[10px] text-destructive ml-auto">Invalid JSON</span>
                {/if}
              </div>
              <textarea
                id="field-{i}"
                class="w-full bg-muted/20 border rounded-md px-3 py-2 text-xs text-foreground font-mono outline-none resize-y focus:border-primary/60 transition-colors placeholder:text-muted-foreground/50
                  {jsonErrors[i] ? 'border-destructive/60' : modified ? 'border-warning' : 'border-border/60'}"
                bind:value={fieldTexts[i]}
                oninput={() => handleJsonInput(i)}
                placeholder={placeholder || '{"key": "value"}'}
                rows="4"
                spellcheck="false"
                autocomplete="off"
                style="min-height: 80px; max-height: 300px;"
              ></textarea>
            </div>

          {:else}
            <input
              id="field-{i}"
              type="text"
              class="w-full bg-muted/20 border rounded-md px-3 py-1.5 text-xs text-foreground outline-none focus:border-primary/60 transition-colors placeholder:text-muted-foreground/50
                {category === 'identifier' || category === 'network' || category === 'geometric' ? 'font-mono' : ''}
                {modified ? 'border-warning' : 'border-border/60'}"
              bind:value={fieldTexts[i]}
              placeholder={placeholder || (mode === 'add' && isAuto ? 'auto-generated (leave empty)' : 'NULL')}
              spellcheck="false"
              autocomplete="off"
            />
          {/if}

          <!-- Modified hint -->
          {#if modified && mode === 'edit'}
            <div class="text-[10px] text-warning mt-0.5">
              was: {originalTexts[i] || 'NULL'}
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <Dialog.Footer class="shrink-0 border-t border-border/60 px-5 py-3 flex items-center justify-between">
      <div class="text-xs text-muted-foreground">
        {#if mode === 'edit' && modifiedCount > 0}
          <span class="text-warning">{modifiedCount} field{modifiedCount > 1 ? 's' : ''} modified</span>
        {:else if mode === 'add'}
          <span class="text-muted-foreground/60">Ctrl+Enter to confirm</span>
        {:else}
          <span></span>
        {/if}
      </div>
      <div class="flex items-center gap-2">
        <Button variant="outline" size="sm" onclick={() => (open = false)}>Cancel</Button>
        <Button
          variant={mode === 'add' ? 'default' : 'default'}
          size="sm"
          onclick={handleConfirm}
          class={mode === 'add' ? 'bg-success hover:bg-success/90 text-success-foreground' : ''}
        >
          {mode === 'add' ? 'Insert Row' : 'Save Changes'}
        </Button>
      </div>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
