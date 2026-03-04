<script lang="ts">
  import type { CellValue } from '$lib/types';
  import { cellValueToEditText, parseInputToCellValue } from '$lib/sql-utils';

  let {
    value,
    dataType,
    onconfirm,
    oncancel,
    ontab,
  }: {
    value: CellValue;
    dataType: string;
    onconfirm: (newValue: CellValue) => void;
    oncancel: () => void;
    ontab?: (shift: boolean) => void;
  } = $props();

  let inputRef = $state<HTMLInputElement | null>(null);
  // svelte-ignore state_referenced_locally
  let textValue = $state(cellValueToEditText(value));
  const isBool = $derived(dataType.toLowerCase() === 'bool');
  const isBytes = $derived(dataType.toLowerCase() === 'bytea');

  function confirm() {
    const newValue = parseInputToCellValue(textValue, dataType);
    onconfirm(newValue);
  }

  function toggleBool() {
    const current = value !== 'Null' && 'Bool' in value ? value.Bool : false;
    onconfirm({ Bool: !current });
  }

  function setNull() {
    onconfirm('Null');
  }

  function handleKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      confirm();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      oncancel();
    } else if (e.key === 'Tab') {
      e.preventDefault();
      confirm();
      ontab?.(e.shiftKey);
    }
  }

  $effect(() => {
    if (inputRef) {
      inputRef.focus();
      inputRef.select();
    }
  });
</script>

{#if isBytes}
  <div class="flex items-center gap-1 h-full px-1">
    <span class="text-text-dim italic text-[10px]">bytes (read-only)</span>
    <button
      class="text-[10px] px-1 py-0.5 rounded bg-accent/50 text-muted-foreground hover:text-foreground"
      onclick={oncancel}
      onkeydown={handleKeydown}
    >Esc</button>
  </div>
{:else if isBool}
  <!-- svelte-ignore a11y_autofocus -->
  <div class="flex items-center gap-1 h-full px-1">
    <button
      class="text-xs px-1.5 py-0.5 rounded transition-colors
        {value !== 'Null' && 'Bool' in value && value.Bool
          ? 'bg-success/20 text-success'
          : 'bg-destructive/20 text-destructive'}"
      onclick={toggleBool}
      onkeydown={handleKeydown}
      autofocus
    >
      {value !== 'Null' && 'Bool' in value ? String(value.Bool) : 'NULL'}
    </button>
    <button
      class="text-[10px] px-1 py-0.5 rounded text-text-dim hover:text-foreground hover:bg-accent/50"
      onclick={setNull}
      onkeydown={handleKeydown}
    >NULL</button>
  </div>
{:else}
  <div class="flex items-center h-full gap-0.5 px-0.5">
    <input
      bind:this={inputRef}
      bind:value={textValue}
      onkeydown={handleKeydown}
      onblur={confirm}
      class="flex-1 min-w-0 h-full bg-transparent text-xs outline-none text-foreground px-1 placeholder:text-text-dim"
      placeholder={value === 'Null' ? 'NULL' : ''}
      spellcheck="false"
      autocomplete="off"
    />
    <button
      class="shrink-0 text-[10px] px-1 py-0.5 rounded text-text-dim hover:text-foreground hover:bg-accent/50"
      onmousedown={(e) => { e.preventDefault(); setNull(); }}
    >NULL</button>
  </div>
{/if}
