<script lang="ts">
  import { getAppState } from '$lib/stores';
  import type { ColumnDef, FilterOperator, TableFilter } from '$lib/types';
  import { Search, X, RefreshCw, Calendar as CalendarIcon, Check, Braces } from '@lucide/svelte';
  import { Calendar } from '$lib/components/ui/calendar';
  import * as Popover from '$lib/components/ui/popover';
  import { CalendarDate, type DateValue } from '@internationalized/date';

  let {
    columns,
    filters,
    rawSqlFilter = '',
    tabId,
    onrefresh,
  }: {
    columns: ColumnDef[];
    filters: TableFilter[];
    rawSqlFilter?: string;
    tabId: string;
    onrefresh?: () => void;
  } = $props();

  const app = getAppState();

  // ── Mode toggle ──
  let sqlMode = $state(false);
  let sqlInputRef = $state<HTMLInputElement | null>(null);
  let sqlText = $state('');

  // ── SQL autocomplete state ──
  let sqlShowSuggestions = $state(false);
  let sqlHighlightedIndex = $state(0);
  let sqlCurrentWord = $state('');
  let sqlWordStart = $state(0);
  let sqlDropdownRef = $state<HTMLDivElement | null>(null);

  // SQL keywords for context (shown after column names)
  const SQL_OPERATORS = ['=', '!=', '<>', '>', '<', '>=', '<=', 'LIKE', 'ILIKE', 'IN', 'NOT IN', 'IS NULL', 'IS NOT NULL', 'BETWEEN', 'AND', 'OR', 'NOT'];

  const sqlSuggestions = $derived.by(() => {
    const word = sqlCurrentWord.toLowerCase();
    if (!word) return [];

    // Suggest columns first, then SQL keywords
    const colMatches = columns
      .filter(c => c.name.toLowerCase().includes(word))
      .map(c => ({ key: c.name, label: c.name, hint: c.data_type, type: 'column' as const }));

    const kwMatches = SQL_OPERATORS
      .filter(kw => kw.toLowerCase().startsWith(word))
      .map(kw => ({ key: kw, label: kw, hint: '', type: 'keyword' as const }));

    return [...colMatches, ...kwMatches].slice(0, 12);
  });

  // Scroll highlighted suggestion into view
  $effect(() => {
    if (sqlShowSuggestions && sqlDropdownRef) {
      const items = sqlDropdownRef.querySelectorAll('[data-sql-item]');
      items[sqlHighlightedIndex]?.scrollIntoView({ block: 'nearest' });
    }
  });

  function extractCurrentWord() {
    if (!sqlInputRef) return;
    const pos = sqlInputRef.selectionStart ?? sqlText.length;
    // Walk backward to find word start (letters, digits, underscore, dot)
    let start = pos;
    while (start > 0 && /[\w.]/.test(sqlText[start - 1])) start--;
    const word = sqlText.slice(start, pos);
    sqlCurrentWord = word;
    sqlWordStart = start;
    sqlHighlightedIndex = 0;
    // Show suggestions if there's a word — the template also checks sqlSuggestions.length
    sqlShowSuggestions = word.length > 0;
  }

  function insertSqlSuggestion(item: { key: string; type: 'column' | 'keyword' }) {
    const pos = sqlInputRef?.selectionStart ?? sqlText.length;
    const before = sqlText.slice(0, sqlWordStart);
    const after = sqlText.slice(pos);
    // Quote column names that need it (contain spaces, uppercase, reserved words)
    const insertText = item.type === 'column' && /[^a-z0-9_]/.test(item.key)
      ? `"${item.key}"`
      : item.key;
    sqlText = before + insertText + (after.startsWith(' ') ? after : ' ' + after);
    sqlShowSuggestions = false;
    const newPos = before.length + insertText.length + 1;
    requestAnimationFrame(() => {
      sqlInputRef?.focus();
      sqlInputRef?.setSelectionRange(newPos, newPos);
    });
  }

  // ── SQL syntax highlighting ──
  const SQL_KEYWORDS = new Set(['AND', 'OR', 'NOT', 'IS', 'NULL', 'IN', 'BETWEEN', 'LIKE', 'ILIKE', 'EXISTS', 'ANY', 'ALL', 'TRUE', 'FALSE']);
  const columnNameSet = $derived(new Set(columns.map(c => c.name.toLowerCase())));

  type TokenType = 'column' | 'keyword' | 'string' | 'operator' | 'number' | 'default';

  function tokenizeSql(text: string): { text: string; type: TokenType }[] {
    const tokens: { text: string; type: TokenType }[] = [];
    let i = 0;
    while (i < text.length) {
      // Whitespace
      if (/\s/.test(text[i])) {
        let start = i;
        while (i < text.length && /\s/.test(text[i])) i++;
        tokens.push({ text: text.slice(start, i), type: 'default' });
        continue;
      }
      // String literal
      if (text[i] === "'") {
        let start = i;
        i++;
        while (i < text.length && text[i] !== "'") i++;
        if (i < text.length) i++; // closing quote
        tokens.push({ text: text.slice(start, i), type: 'string' });
        continue;
      }
      // Quoted identifier
      if (text[i] === '"') {
        let start = i;
        i++;
        while (i < text.length && text[i] !== '"') i++;
        if (i < text.length) i++;
        const name = text.slice(start + 1, i - 1);
        tokens.push({ text: text.slice(start, i), type: columnNameSet.has(name.toLowerCase()) ? 'column' : 'default' });
        continue;
      }
      // Operators
      if (/[=<>!]/.test(text[i])) {
        let start = i;
        while (i < text.length && /[=<>!]/.test(text[i])) i++;
        tokens.push({ text: text.slice(start, i), type: 'operator' });
        continue;
      }
      // Numbers
      if (/\d/.test(text[i])) {
        let start = i;
        while (i < text.length && /[\d.]/.test(text[i])) i++;
        tokens.push({ text: text.slice(start, i), type: 'number' });
        continue;
      }
      // Words (identifiers / keywords)
      if (/[\w]/.test(text[i])) {
        let start = i;
        while (i < text.length && /[\w.]/.test(text[i])) i++;
        const word = text.slice(start, i);
        if (SQL_KEYWORDS.has(word.toUpperCase())) {
          tokens.push({ text: word, type: 'keyword' });
        } else if (columnNameSet.has(word.toLowerCase())) {
          tokens.push({ text: word, type: 'column' });
        } else {
          tokens.push({ text: word, type: 'default' });
        }
        continue;
      }
      // Other chars (parens, commas, etc.)
      tokens.push({ text: text[i], type: 'default' });
      i++;
    }
    return tokens;
  }

  const TOKEN_CLASSES: Record<TokenType, string> = {
    column: 'text-sky-400',
    keyword: 'text-primary font-medium',
    string: 'text-success',
    operator: 'text-warning',
    number: 'text-amber-300',
    default: 'text-foreground',
  };

  const highlightedHtml = $derived.by(() => {
    if (!sqlText) return '';
    const tokens = tokenizeSql(sqlText);
    return tokens.map(t => {
      const escaped = t.text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
      return `<span class="${TOKEN_CLASSES[t.type]}">${escaped}</span>`;
    }).join('');
  });

  // Initialize sqlMode from existing rawSqlFilter
  $effect(() => {
    if (rawSqlFilter) {
      sqlMode = true;
      sqlText = rawSqlFilter;
    }
  });

  function toggleMode() {
    sqlMode = !sqlMode;
    if (sqlMode) {
      // Convert existing chips to SQL text
      if (filters.length > 0) {
        const clauses = filters.map(f => app.filterToSql(f)).filter(Boolean);
        sqlText = clauses.join(' AND ');
      } else {
        sqlText = '';
      }
      sqlShowSuggestions = false;
      requestAnimationFrame(() => {
        sqlInputRef?.focus();
        // Place cursor at end
        if (sqlInputRef && sqlText) {
          sqlInputRef.setSelectionRange(sqlText.length, sqlText.length);
        }
      });
    } else {
      if (rawSqlFilter) {
        app.updateDataTabRawFilter(tabId, undefined);
      }
      sqlText = '';
    }
  }

  function applySqlFilter() {
    const text = sqlText.trim();
    sqlShowSuggestions = false;
    if (!text) {
      app.updateDataTabRawFilter(tabId, undefined);
    } else {
      app.updateDataTabRawFilter(tabId, text);
    }
  }

  function clearSqlFilter() {
    sqlText = '';
    sqlShowSuggestions = false;
    app.updateDataTabRawFilter(tabId, undefined);
    sqlInputRef?.focus();
  }

  function handleSqlKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (sqlShowSuggestions && sqlSuggestions.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        sqlHighlightedIndex = Math.min(sqlHighlightedIndex + 1, sqlSuggestions.length - 1);
        return;
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        sqlHighlightedIndex = Math.max(sqlHighlightedIndex - 1, 0);
        return;
      } else if (e.key === 'Tab' || (e.key === 'Enter' && sqlSuggestions.length > 0)) {
        e.preventDefault();
        insertSqlSuggestion(sqlSuggestions[sqlHighlightedIndex]);
        return;
      } else if (e.key === 'Escape') {
        e.preventDefault();
        sqlShowSuggestions = false;
        return;
      }
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      applySqlFilter();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      sqlText = rawSqlFilter || '';
      sqlShowSuggestions = false;
      sqlInputRef?.blur();
    }
  }

  function handleSqlInput() {
    extractCurrentWord();
  }

  function handleSqlFocus() {
    extractCurrentWord();
  }

  let sqlBlurTimeout: ReturnType<typeof setTimeout> | null = null;

  function handleSqlBlur() {
    sqlBlurTimeout = setTimeout(() => { sqlShowSuggestions = false; }, 150);
  }

  function cancelSqlBlur() {
    if (sqlBlurTimeout) {
      clearTimeout(sqlBlurTimeout);
      sqlBlurTimeout = null;
    }
  }

  // ── Quick filter state (existing) ──
  const OPERATORS: { value: FilterOperator; label: string }[] = [
    { value: 'equals', label: '=' },
    { value: 'not_equals', label: '≠' },
    { value: 'gt', label: '>' },
    { value: 'lt', label: '<' },
    { value: 'gte', label: '≥' },
    { value: 'lte', label: '≤' },
    { value: 'contains', label: 'contains' },
    { value: 'starts_with', label: 'starts with' },
    { value: 'is_null', label: 'is null' },
    { value: 'is_not_null', label: 'is not null' },
  ];

  const NO_VALUE_OPS: FilterOperator[] = ['is_null', 'is_not_null'];

  function opLabel(op: FilterOperator): string {
    return OPERATORS.find(o => o.value === op)?.label ?? op;
  }

  let searchText = $state('');
  let selectedColumn = $state<string | null>(null);
  let selectedOperator = $state<FilterOperator | null>(null);
  let showSuggestions = $state(false);
  let highlightedIndex = $state(0);
  let filterInputRef = $state<HTMLInputElement | null>(null);

  const TIMESTAMP_TYPES = ['timestamp', 'timestamptz', 'timestamp without time zone', 'timestamp with time zone', 'date'];

  const selectedColumnIsTimestamp = $derived.by(() => {
    if (!selectedColumn) return false;
    const col = columns.find(c => c.name === selectedColumn);
    return col ? TIMESTAMP_TYPES.includes(col.data_type.toLowerCase()) : false;
  });

  let calendarOpen = $state(false);
  let calendarValue = $state<DateValue | undefined>(undefined);
  let timeHour = $state('00');
  let timeMinute = $state('00');

  let dropdownRef = $state<HTMLDivElement | null>(null);

  // Scroll highlighted item into view when navigating with arrow keys
  $effect(() => {
    if (showSuggestions && dropdownRef) {
      const items = dropdownRef.querySelectorAll('[data-dropdown-item]');
      items[highlightedIndex]?.scrollIntoView({ block: 'nearest' });
    }
  });

  const filteredColumns = $derived.by(() => {
    if (selectedColumn) return [];
    const text = searchText.toLowerCase();
    return columns.filter(c =>
      !text || c.name.toLowerCase().includes(text)
    );
  });

  type SuggestionStep = 'column' | 'operator' | 'value';
  const currentStep = $derived<SuggestionStep>(
    !selectedColumn ? 'column' : !selectedOperator ? 'operator' : 'value'
  );

  const dropdownItems = $derived.by(() => {
    if (currentStep === 'column') return filteredColumns.map(c => ({ key: c.name, label: c.name, hint: c.data_type }));
    if (currentStep === 'operator') return OPERATORS.map(o => ({ key: o.value, label: o.label, hint: '' }));
    return [];
  });

  function selectFilterColumn(colName: string) {
    selectedColumn = colName;
    selectedOperator = null;
    searchText = '';
    highlightedIndex = 0;
    showSuggestions = true;
    filterInputRef?.focus();
  }

  function selectFilterOperator(op: FilterOperator) {
    selectedOperator = op;
    searchText = '';
    highlightedIndex = 0;
    showSuggestions = false;

    if (NO_VALUE_OPS.includes(op)) {
      const newFilters = [...filters, { column: selectedColumn!, operator: op, value: '' }];
      app.updateDataTabFilters(tabId, newFilters);
      selectedColumn = null;
      selectedOperator = null;
    } else {
      filterInputRef?.focus();
    }
  }

  function handleDropdownSelect(key: string) {
    if (currentStep === 'column') selectFilterColumn(key);
    else if (currentStep === 'operator') selectFilterOperator(key as FilterOperator);
  }

  function addFilterChip() {
    const text = searchText.trim();
    if (!text || !selectedColumn || !selectedOperator) return;
    const newFilters = [...filters, { column: selectedColumn, operator: selectedOperator, value: text }];
    app.updateDataTabFilters(tabId, newFilters);
    selectedColumn = null;
    selectedOperator = null;
    searchText = '';
    showSuggestions = false;
  }

  function removeFilterChip(index: number) {
    const newFilters = filters.filter((_, i) => i !== index);
    app.updateDataTabFilters(tabId, newFilters);
  }

  function editFilterChip(index: number) {
    cancelBlur();
    const filter = filters[index];
    removeFilterChip(index);
    selectedColumn = filter.column;
    selectedOperator = filter.operator;
    searchText = NO_VALUE_OPS.includes(filter.operator) ? '' : filter.value;
    showSuggestions = false;
    requestAnimationFrame(() => filterInputRef?.focus());
  }

  function clearAllFilters() {
    app.updateDataTabFilters(tabId, []);
    searchText = '';
    selectedColumn = null;
    selectedOperator = null;
    showSuggestions = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Enter') {
      if (showSuggestions && dropdownItems.length > 0 && currentStep !== 'value') {
        e.preventDefault();
        handleDropdownSelect(dropdownItems[highlightedIndex].key);
      } else {
        addFilterChip();
      }
    } else if (e.key === 'Tab' && showSuggestions && dropdownItems.length > 0 && currentStep !== 'value') {
      e.preventDefault();
      handleDropdownSelect(dropdownItems[highlightedIndex].key);
    } else if (e.key === 'Escape') {
      if (selectedOperator) {
        selectedOperator = null;
        showSuggestions = true;
      } else if (selectedColumn) {
        selectedColumn = null;
        showSuggestions = true;
      } else {
        showSuggestions = false;
      }
      searchText = '';
    } else if (e.key === 'ArrowDown' && showSuggestions && dropdownItems.length > 0) {
      e.preventDefault();
      highlightedIndex = Math.min(highlightedIndex + 1, dropdownItems.length - 1);
    } else if (e.key === 'ArrowUp' && showSuggestions && dropdownItems.length > 0) {
      e.preventDefault();
      highlightedIndex = Math.max(highlightedIndex - 1, 0);
    } else if (e.key === 'Backspace' && searchText === '') {
      if (selectedOperator) {
        selectedOperator = null;
        showSuggestions = true;
      } else if (selectedColumn) {
        selectedColumn = null;
        showSuggestions = true;
      } else if (filters.length > 0) {
        removeFilterChip(filters.length - 1);
      }
    }
  }

  function handleInput() {
    if (currentStep === 'column') {
      highlightedIndex = 0;
      showSuggestions = true;
    }
  }

  function handleFocus() {
    if (currentStep !== 'value') showSuggestions = true;
  }

  let blurTimeout: ReturnType<typeof setTimeout> | null = null;

  function handleBlur() {
    blurTimeout = setTimeout(() => { showSuggestions = false; }, 150);
  }

  function cancelBlur() {
    if (blurTimeout) {
      clearTimeout(blurTimeout);
      blurTimeout = null;
    }
  }
</script>

{#if sqlMode}
  <!-- ══ SQL WHERE mode ══ -->
  <Braces class="h-3 w-3 text-primary shrink-0" />

  <span class="text-[10px] text-primary font-medium shrink-0">WHERE</span>

  <div class="relative flex-1 min-w-[80px]">
    <!-- Syntax-highlighted overlay (behind input) -->
    {#if sqlText}
      <div
        class="absolute inset-0 text-xs font-mono whitespace-pre pointer-events-none overflow-hidden flex items-center"
        aria-hidden="true"
      >{@html highlightedHtml}</div>
    {/if}
    <input
      bind:this={sqlInputRef}
      type="text"
      placeholder='e.g. age > 25 AND status = &#39;active&#39;'
      class="w-full bg-transparent text-xs font-mono outline-none placeholder:text-muted-foreground/40 relative
        {sqlText ? 'text-transparent caret-foreground' : 'text-foreground'}"
      bind:value={sqlText}
      oninput={handleSqlInput}
      onfocus={handleSqlFocus}
      onblur={handleSqlBlur}
      onkeydown={handleSqlKeydown}
    />

    {#if sqlShowSuggestions && sqlSuggestions.length > 0}
      <div
        bind:this={sqlDropdownRef}
        class="absolute left-0 top-full mt-1 z-50 w-[220px] max-h-[220px] overflow-y-auto rounded-lg border border-border/60 bg-popover p-1 shadow-xl shadow-black/30"
      >
        {#each sqlSuggestions as item, i}
          <button
            data-sql-item
            class="flex w-full items-center justify-between rounded-sm px-2 py-1 text-xs transition-colors {i === sqlHighlightedIndex ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50'}"
            onmousedown={() => { cancelSqlBlur(); insertSqlSuggestion(item); }}
          >
            <span class="{item.type === 'keyword' ? 'text-primary font-medium' : ''}">{item.label}</span>
            {#if item.hint}
              <span class="text-muted-foreground/50 text-[10px]">{item.hint}</span>
            {/if}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  {#if sqlText.trim() && sqlText.trim() !== rawSqlFilter}
    <button
      class="p-1 rounded text-primary hover:bg-primary/10 transition-colors shrink-0"
      onclick={applySqlFilter}
      title="Apply SQL filter (Enter)"
    >
      <Check class="h-3.5 w-3.5" />
    </button>
  {/if}

  {#if rawSqlFilter}
    <button class="text-muted-foreground hover:text-foreground shrink-0" onclick={clearSqlFilter} title="Clear SQL filter">
      <X class="h-3 w-3" />
    </button>
  {/if}

{:else}
  <!-- ══ Quick filter mode ══ -->
  <Search class="h-3 w-3 text-text-dim shrink-0" />

  {#each filters as filter, i}
    <button
      class="inline-flex items-center gap-0.5 bg-primary/15 text-[11px] rounded px-1.5 py-0.5 shrink-0 hover:bg-primary/25 transition-colors cursor-pointer group/chip"
      onclick={() => editFilterChip(i)}
      title="Click to edit"
    >
      <span class="text-muted-foreground">{filter.column}</span>
      <span class="text-primary font-medium">{opLabel(filter.operator)}</span>
      {#if !NO_VALUE_OPS.includes(filter.operator)}
        <span class="text-foreground">{filter.value}</span>
      {/if}
      <span
        class="text-muted-foreground hover:text-destructive ml-0.5"
        role="button"
        tabindex={0}
        onclick={(e: MouseEvent) => { e.stopPropagation(); removeFilterChip(i); }}
        onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter') { e.stopPropagation(); removeFilterChip(i); } }}
      >
        <X class="h-2.5 w-2.5" />
      </span>
    </button>
  {/each}

  {#if selectedColumn}
    <button
      class="text-[11px] text-muted-foreground bg-secondary rounded px-1.5 py-0.5 shrink-0 hover:bg-secondary/80 transition-colors cursor-pointer"
      onclick={() => { cancelBlur(); selectedColumn = null; selectedOperator = null; searchText = ''; showSuggestions = true; filterInputRef?.focus(); }}
      title="Click to change column"
    >{selectedColumn}</button>
  {/if}
  {#if selectedOperator}
    <button
      class="text-[11px] text-primary bg-secondary rounded px-1.5 py-0.5 shrink-0 hover:bg-secondary/80 transition-colors cursor-pointer"
      onclick={() => { cancelBlur(); selectedOperator = null; searchText = ''; showSuggestions = true; filterInputRef?.focus(); }}
      title="Click to change operator"
    >{opLabel(selectedOperator)}</button>
  {/if}

  <div class="relative flex-1 min-w-[80px]">
    {#if currentStep === 'value' && selectedColumnIsTimestamp}
      <div class="flex items-center gap-1">
        <input
          bind:this={filterInputRef}
          type="text"
          placeholder="Value, then Enter"
          class="flex-1 bg-transparent text-xs outline-none placeholder:text-muted-foreground/40"
          bind:value={searchText}
          onkeydown={handleKeydown}
        />
        <Popover.Root bind:open={calendarOpen}>
          <Popover.Trigger
            class="p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors cursor-pointer"
            title="Pick a date"
          >
            <CalendarIcon class="h-3.5 w-3.5" />
          </Popover.Trigger>
          <Popover.Content class="w-auto p-0" align="start" sideOffset={8}>
            <Calendar
              type="single"
              bind:value={calendarValue}
              captionLayout="dropdown"
            />
            <div class="border-t border-border px-3 py-2.5 flex items-center justify-between">
              <div class="flex items-center gap-1.5 text-sm text-muted-foreground">
                <span class="text-xs">Time</span>
                <input
                  type="text"
                  maxlength={2}
                  bind:value={timeHour}
                  class="w-10 h-7 bg-secondary border border-border rounded-md px-2 py-1 text-center text-sm tabular-nums text-foreground outline-none focus:ring-1 focus:ring-primary [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                  placeholder="HH"
                  onfocus={(e: FocusEvent) => (e.target as HTMLInputElement).select()}
                  onblur={() => { timeHour = String(Math.max(0, Math.min(23, parseInt(timeHour) || 0))).padStart(2, '0'); }}
                />
                <span class="text-foreground font-semibold">:</span>
                <input
                  type="text"
                  maxlength={2}
                  bind:value={timeMinute}
                  class="w-10 h-7 bg-secondary border border-border rounded-md px-2 py-1 text-center text-sm tabular-nums text-foreground outline-none focus:ring-1 focus:ring-primary [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                  placeholder="MM"
                  onfocus={(e: FocusEvent) => (e.target as HTMLInputElement).select()}
                  onblur={() => { timeMinute = String(Math.max(0, Math.min(59, parseInt(timeMinute) || 0))).padStart(2, '0'); }}
                />
              </div>
              <button
                class="text-xs font-medium bg-primary text-primary-foreground rounded-md px-3 py-1.5 hover:bg-primary/90 transition-colors"
                onclick={() => {
                  if (calendarValue) {
                    const y = String(calendarValue.year).padStart(4, '0');
                    const m = String(calendarValue.month).padStart(2, '0');
                    const d = String(calendarValue.day).padStart(2, '0');
                    searchText = `${y}-${m}-${d} ${timeHour}:${timeMinute}`;
                  }
                  calendarOpen = false;
                  filterInputRef?.focus();
                }}
              >
                Apply
              </button>
            </div>
          </Popover.Content>
        </Popover.Root>
      </div>
    {:else}
      <input
        bind:this={filterInputRef}
        type="text"
        placeholder={currentStep === 'column' ? 'Filter...' : currentStep === 'operator' ? 'Operator...' : 'Value, then Enter'}
        class="w-full bg-transparent text-xs outline-none placeholder:text-muted-foreground/40"
        bind:value={searchText}
        oninput={handleInput}
        onfocus={handleFocus}
        onblur={handleBlur}
        onkeydown={handleKeydown}
      />
    {/if}

    {#if showSuggestions && dropdownItems.length > 0}
      <div
        bind:this={dropdownRef}
        class="absolute left-0 top-full mt-1 z-50 w-[220px] max-h-[220px] overflow-y-auto rounded-lg border border-border/60 bg-popover p-1 shadow-xl shadow-black/30"
      >
        <div class="px-2 py-0.5 text-[10px] text-muted-foreground/50 uppercase tracking-wider">
          {currentStep === 'column' ? 'Columns' : 'Operator'}
        </div>
        {#each dropdownItems as item, i}
          <button
            data-dropdown-item
            class="flex w-full items-center justify-between rounded-sm px-2 py-1 text-xs transition-colors {i === highlightedIndex ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50'}"
            onmousedown={(e) => { e.preventDefault(); handleDropdownSelect(item.key); }}
          >
            <span>{item.label}</span>
            {#if item.hint}
              <span class="text-muted-foreground/50 text-[10px]">{item.hint}</span>
            {/if}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  {#if currentStep === 'value' && searchText.trim()}
    <button
      class="p-1 rounded text-primary hover:bg-primary/10 transition-colors shrink-0"
      onclick={addFilterChip}
      title="Apply filter"
    >
      <Check class="h-3.5 w-3.5" />
    </button>
  {/if}

  {#if filters.length > 0}
    <button class="text-muted-foreground hover:text-foreground shrink-0" onclick={clearAllFilters} title="Clear all filters">
      <X class="h-3 w-3" />
    </button>
  {/if}
{/if}

<!-- ══ Mode toggle + Refresh (always visible) ══ -->
<div class="w-px h-4 bg-border/60 shrink-0"></div>

<button
  class="px-1 py-0.5 rounded text-[10px] font-mono transition-colors shrink-0
    {sqlMode ? 'text-primary bg-primary/10 hover:bg-primary/20' : 'text-muted-foreground hover:text-foreground hover:bg-accent/50'}"
  onclick={toggleMode}
  title={sqlMode ? 'Switch to quick filter' : 'Switch to SQL WHERE filter'}
>
  SQL
</button>

{#if onrefresh}
  <button
    class="p-1 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors shrink-0"
    onclick={onrefresh}
    title="Refresh table"
  >
    <RefreshCw class="h-3 w-3" />
  </button>
{/if}
