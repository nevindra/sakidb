<script lang="ts">
  import type { CellValue } from '$lib/types';
  import { cellValueToEditText, parseInputToCellValue } from '$lib/sql-utils';
  import { getTypeCategory, getTypePlaceholder } from '$lib/type-utils';
  import { CalendarDate, type DateValue } from '@internationalized/date';
  import { uuidv7 } from 'uuidv7';
  import * as Popover from '$lib/components/ui/popover';
  import { Calendar } from '$lib/components/ui/calendar';
  import Switch from '$lib/components/ui/switch/switch.svelte';

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

  let inputRef = $state<HTMLInputElement | HTMLTextAreaElement | null>(null);
  // svelte-ignore state_referenced_locally
  let textValue = $state(cellValueToEditText(value));
  let jsonError = $state(false);

  const t = $derived(dataType.toLowerCase().replace(/\s*\(.*\)$/, ''));
  const category = $derived(getTypeCategory(dataType));
  const placeholder = $derived(getTypePlaceholder(dataType));
  const isBool = $derived(category === 'boolean');
  const isUuid = $derived(t === 'uuid');
  const isBytes = $derived(t === 'bytea');
  const isJson = $derived(t === 'json' || t === 'jsonb');
  const isDate = $derived(t === 'date');
  const isTimestamp = $derived(t === 'timestamp' || t === 'timestamptz'
    || t === 'timestamp without time zone' || t === 'timestamp with time zone');
  const isTime = $derived(t === 'time' || t === 'timetz'
    || t === 'time without time zone' || t === 'time with time zone');
  const isTemporal = $derived(isDate || isTimestamp || isTime);

  // ── Bool state ──
  let boolChecked = $state(value !== 'Null' && 'Bool' in value ? value.Bool : false);
  let boolIsNull = $state(value === 'Null');

  // ── Date/time state ──
  let manualInput = $state('');

  // Parse manual input into a CalendarDate for the calendar to highlight
  const calendarValue = $derived.by((): DateValue | undefined => {
    const match = manualInput.match(/(\d{4})-(\d{1,2})-(\d{1,2})/);
    if (!match) return undefined;
    try {
      return new CalendarDate(parseInt(match[1]), parseInt(match[2]), parseInt(match[3]));
    } catch {
      return undefined;
    }
  });

  // Initialize manual input from existing value
  $effect(() => {
    manualInput = textValue.trim();
  });

  // ── JSON state ──
  const prettyJson = $derived.by(() => {
    if (!isJson) return textValue;
    try {
      return JSON.stringify(JSON.parse(textValue), null, 2);
    } catch {
      return textValue;
    }
  });
  // svelte-ignore state_referenced_locally
  let jsonText = $state('');
  $effect(() => { jsonText = prettyJson; });

  function confirm() {
    const newValue = parseInputToCellValue(textValue, dataType);
    onconfirm(newValue);
  }

  function confirmJson() {
    const trimmed = jsonText.trim();
    if (!trimmed || trimmed.toLowerCase() === 'null') {
      onconfirm('Null');
      return;
    }
    try {
      JSON.parse(trimmed);
      jsonError = false;
    } catch {
      jsonError = true;
    }
    onconfirm({ Json: trimmed });
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

  function handleTemporalKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      oncancel();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      confirmTemporal();
    }
  }

  function handleJsonKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      oncancel();
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      confirmJson();
    } else if (e.key === 'Tab') {
      // Insert 2 spaces for indentation
      e.preventDefault();
      const ta = e.target as HTMLTextAreaElement;
      const start = ta.selectionStart;
      const end = ta.selectionEnd;
      jsonText = jsonText.substring(0, start) + '  ' + jsonText.substring(end);
      requestAnimationFrame(() => {
        ta.selectionStart = ta.selectionEnd = start + 2;
      });
    }
  }

  function handleCalendarSelect(date: DateValue | undefined) {
    if (!date) return;
    const dateStr = `${date.year}-${String(date.month).padStart(2, '0')}-${String(date.day).padStart(2, '0')}`;
    if (isTimestamp) {
      // Preserve existing time portion if present
      const timePart = manualInput.split(/[T ]/)[1] ?? '';
      manualInput = timePart ? `${dateStr} ${timePart}` : dateStr;
    } else {
      manualInput = dateStr;
    }
  }

  function confirmTemporal() {
    textValue = manualInput;
    confirm();
  }

  $effect(() => {
    if (inputRef) {
      inputRef.focus();
      if (inputRef instanceof HTMLInputElement) {
        inputRef.select();
      }
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
  <!-- Boolean — popover with switch -->
  <div class="flex items-center h-full px-1">
    <Popover.Root open={true} onOpenChange={(open) => { if (!open) oncancel(); }}>
      <Popover.Trigger class="h-full flex items-center text-xs text-foreground">
        <span class="text-warning">
          {boolIsNull ? 'NULL' : String(boolChecked)}
        </span>
      </Popover.Trigger>
      <Popover.Content class="w-auto p-0" sideOffset={4} align="start">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="bg-card rounded-md border shadow-xl p-3 min-w-[160px]" onkeydown={(e) => {
          e.stopPropagation();
          if (e.key === 'Escape') { oncancel(); }
          else if (e.key === 'Enter') { onconfirm(boolIsNull ? 'Null' : { Bool: boolChecked }); }
        }}>
          <!-- Switch row -->
          <div class="flex items-center justify-between gap-4">
            <button
              class="text-xs px-2 py-1 rounded transition-colors font-medium
                {boolIsNull ? 'text-text-dim' : boolChecked ? 'text-success' : 'text-destructive'}"
              onclick={() => {
                boolIsNull = false;
                boolChecked = !boolChecked;
              }}
            >
              {boolIsNull ? 'NULL' : boolChecked ? 'true' : 'false'}
            </button>
            <Switch
              checked={boolChecked}
              disabled={boolIsNull}
              onCheckedChange={(v) => { boolIsNull = false; boolChecked = v; }}
            />
          </div>

          <!-- Actions -->
          <div class="flex items-center justify-between mt-3 pt-2 border-t border-border/40">
            <button
              class="text-[10px] px-1.5 py-0.5 rounded transition-colors
                {boolIsNull ? 'bg-accent/50 text-foreground' : 'text-text-dim hover:text-foreground hover:bg-accent/50'}"
              onclick={() => { boolIsNull = true; }}
            >NULL</button>
            <div class="flex items-center gap-1.5">
              <button
                class="text-[10px] px-1.5 py-0.5 rounded text-text-dim hover:text-foreground hover:bg-accent/50"
                onclick={oncancel}
              >Cancel</button>
              <button
                class="text-[10px] px-2 py-0.5 rounded bg-primary text-primary-foreground hover:bg-primary/90"
                onclick={() => onconfirm(boolIsNull ? 'Null' : { Bool: boolChecked })}
              >OK</button>
            </div>
          </div>
        </div>
      </Popover.Content>
    </Popover.Root>
  </div>

{:else if isTemporal}
  <!-- Date / Timestamp / Time — calendar popover opens directly -->
  <div class="flex items-center h-full px-1">
    <Popover.Root open={true} onOpenChange={(open) => { if (!open) oncancel(); }}>
      <Popover.Trigger class="h-full flex items-center text-xs text-foreground">
        <span class="text-success">{textValue || placeholder}</span>
      </Popover.Trigger>
      <Popover.Content class="w-auto p-0" sideOffset={4} align="start">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="bg-card rounded-md border shadow-xl" onkeydown={handleTemporalKeydown}>
          {#if isDate || isTimestamp}
            <Calendar
              type="single"
              value={calendarValue}
              onValueChange={handleCalendarSelect}
              class="border-b-0"
            />
          {/if}

          <!-- Manual input for full value (date, time, timestamp with ms, tz, etc.) -->
          <div class="px-3 pb-2 pt-1 {isDate || isTimestamp ? 'border-t border-border/40' : 'pt-3'}">
            <input
              bind:this={inputRef}
              bind:value={manualInput}
              onkeydown={handleTemporalKeydown}
              placeholder={placeholder}
              class="w-full text-xs bg-transparent border border-border/60 rounded px-2 py-1.5 text-foreground outline-none focus:border-primary/60 placeholder:text-text-dim font-mono"
              spellcheck="false"
              autocomplete="off"
            />
          </div>

          <!-- Actions -->
          <div class="flex items-center justify-between px-3 py-2 border-t border-border/40">
            <button
              class="text-[10px] px-1.5 py-0.5 rounded text-text-dim hover:text-foreground hover:bg-accent/50"
              onclick={setNull}
            >NULL</button>
            <div class="flex items-center gap-1.5">
              <button
                class="text-[10px] px-1.5 py-0.5 rounded text-text-dim hover:text-foreground hover:bg-accent/50"
                onclick={oncancel}
              >Cancel</button>
              <button
                class="text-[10px] px-2 py-0.5 rounded bg-primary text-primary-foreground hover:bg-primary/90"
                onclick={confirmTemporal}
              >OK</button>
            </div>
          </div>
        </div>
      </Popover.Content>
    </Popover.Root>
  </div>

{:else if isJson}
  <!-- JSON editor — opens as a floating popover -->
  <div class="flex items-center h-full px-1">
    <Popover.Root open={true} onOpenChange={(open) => { if (!open) oncancel(); }}>
      <Popover.Trigger class="h-full flex items-center text-xs text-foreground font-mono text-primary">
        <span class="truncate max-w-[200px]">{textValue || '{ }'}</span>
      </Popover.Trigger>
      <Popover.Content class="w-[400px] p-0" sideOffset={4} align="start">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="bg-card rounded-md border shadow-xl" onkeydown={handleJsonKeydown}>
          <div class="flex items-center justify-between px-3 py-1.5 border-b border-border/40">
            <span class="text-[10px] text-text-dim font-medium uppercase tracking-wide">JSON Editor</span>
            {#if jsonError}
              <span class="text-[10px] text-destructive">Invalid JSON</span>
            {/if}
          </div>
          <div class="p-2">
            <textarea
              bind:this={inputRef}
              bind:value={jsonText}
              oninput={() => {
                try { JSON.parse(jsonText.trim()); jsonError = false; } catch { jsonError = jsonText.trim().length > 0; }
              }}
              class="w-full bg-muted/30 text-xs text-foreground px-3 py-2 font-mono rounded border outline-none resize-y
                {jsonError ? 'border-destructive/60 focus:border-destructive/80' : 'border-border/60 focus:border-primary/60'}
                placeholder:text-text-dim"
              placeholder={'{"key": "value"}'}
              spellcheck="false"
              autocomplete="off"
              rows="10"
              style="min-height: 120px; max-height: 400px; line-height: 1.5;"
            ></textarea>
          </div>
          <div class="flex items-center justify-between px-3 py-2 border-t border-border/40">
            <div class="flex items-center gap-2">
              <button
                class="text-[10px] px-1.5 py-0.5 rounded text-text-dim hover:text-foreground hover:bg-accent/50"
                onclick={setNull}
              >NULL</button>
              <button
                class="text-[10px] px-1.5 py-0.5 rounded text-text-dim hover:text-foreground hover:bg-accent/50"
                onclick={() => {
                  try {
                    jsonText = JSON.stringify(JSON.parse(jsonText), null, 2);
                    jsonError = false;
                  } catch { jsonError = true; }
                }}
              >Format</button>
              <button
                class="text-[10px] px-1.5 py-0.5 rounded text-text-dim hover:text-foreground hover:bg-accent/50"
                onclick={() => {
                  try {
                    jsonText = JSON.stringify(JSON.parse(jsonText));
                    jsonError = false;
                  } catch { jsonError = true; }
                }}
              >Minify</button>
            </div>
            <div class="flex items-center gap-1.5">
              <span class="text-[9px] text-text-dim">Ctrl+Enter</span>
              <button
                class="text-[10px] px-1.5 py-0.5 rounded text-text-dim hover:text-foreground hover:bg-accent/50"
                onclick={oncancel}
              >Cancel</button>
              <button
                class="text-[10px] px-2 py-0.5 rounded bg-primary text-primary-foreground hover:bg-primary/90"
                onclick={confirmJson}
              >Save</button>
            </div>
          </div>
        </div>
      </Popover.Content>
    </Popover.Root>
  </div>

{:else}
  <!-- Default text input with type-aware placeholder -->
  <div class="flex items-center h-full gap-0.5 px-0.5">
    <input
      bind:this={inputRef}
      bind:value={textValue}
      onkeydown={handleKeydown}
      onblur={confirm}
      class="flex-1 min-w-0 h-full bg-transparent text-xs outline-none text-foreground px-1 placeholder:text-text-dim
        {category === 'identifier' || category === 'network' || category === 'geometric' || category === 'binary' ? 'font-mono' : ''}"
      placeholder={placeholder || (value === 'Null' ? 'NULL' : '')}
      spellcheck="false"
      autocomplete="off"
    />
    {#if isUuid}
      <button
        class="shrink-0 text-[10px] px-1 py-0.5 rounded text-sky-400/80 hover:text-sky-300 hover:bg-accent/50 font-mono"
        onmousedown={(e) => { e.preventDefault(); textValue = uuidv7(); }}
        title="Generate UUID v7"
      >Gen</button>
    {/if}
    <button
      class="shrink-0 text-[10px] px-1 py-0.5 rounded text-text-dim hover:text-foreground hover:bg-accent/50"
      onmousedown={(e) => { e.preventDefault(); setNull(); }}
    >NULL</button>
  </div>
{/if}
