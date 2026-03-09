<script lang="ts">
  import { onMount } from 'svelte';
  import { getAppState } from '$lib/stores';
  import type { QueryTab, QueryResult } from '$lib/types';
  import DataGrid from '../data-view/DataGrid.svelte';
  import QueryToolbar from './QueryToolbar.svelte';
  import QueryResultBar from './QueryResultBar.svelte';
  import ResultTabBar from './ResultTabBar.svelte';
  import ExplainViewer from './ExplainViewer.svelte';
  import CompareView from './CompareView.svelte';
  import { Loader2 } from '@lucide/svelte';
  import type { CompareConfig } from '$lib/types';
  import { EditorView, keymap } from '@codemirror/view';
  import { EditorState, Compartment, Prec } from '@codemirror/state';
  import { basicSetup } from 'codemirror';
  import { sql } from '@codemirror/lang-sql';
  import { autocompletion } from '@codemirror/autocomplete';
  import { oneDark } from '@codemirror/theme-one-dark';
  import { isExplainResult, isExplainText } from '$lib/utils/explain-parser';
  import { ColumnarResultData } from '$lib/types/query-result-data';
  import { getStatementAtCursor, splitStatements } from '$lib/utils/split-statements';
  import { formatSql } from '$lib/utils/sql-format';
  import { createSqlCompletionSource } from '$lib/utils/sql-completion';
  import type { CompletionBundle, EngineType } from '$lib/types';
  import { getDialect } from '$lib/dialects';

  let { tab }: { tab: QueryTab } = $props();

  const app = getAppState();

  // Resolve dialect from connection engine type
  const dialect = $derived.by(() => {
    const engine = app.getSavedConnection(tab.savedConnectionId)?.engine as EngineType | undefined;
    return getDialect(engine ?? 'postgres');
  });

  let editorContainer: HTMLDivElement;
  let editorView: EditorView | null = null;
  let resizing = $state(false);
  let editorHeight = $state(200);
  let hasSelection = $state(false);
  let forceRawView = $state(false);

  // Compartment for dynamically swapping SQL autocompletion
  const completionCompartment = new Compartment();

  // Current completion bundle (eagerly loaded tables + functions)
  let currentBundle: CompletionBundle | null = null;

  // Custom completion source that reads from the current bundle + lazily loads columns
  const completionSource = createSqlCompletionSource(
    () => currentBundle,
    (tableName: string) => app.getTableColumnsForCompletion(
      tab.savedConnectionId, tab.databaseName, tab.schemaName, tableName
    ),
  );

  // ── Derived state ──
  const activeResult = $derived(
    tab.queryResults.length > 0 ? tab.queryResults[tab.activeResultIndex] : null
  );
  const totalExecutionTimeMs = $derived(
    tab.queryResults.reduce((sum, r) => sum + r.execution_time_ms, 0)
  );
  const isExplain = $derived(
    activeResult && !forceRawView
      ? isExplainResultFromAny(activeResult)
      : false
  );

  function isExplainResultFromAny(result: QueryResult | ColumnarResultData): boolean {
    if (result instanceof ColumnarResultData) {
      if (result.columns.length !== 1) return false;
      if (result.columns[0].name.toUpperCase() !== 'QUERY PLAN') return false;
      if (result.row_count === 0) return true;
      const cd = result.columnData[0];
      if (cd.type !== 'text' || cd.nulls[0] !== 0) return false;
      return isExplainText(result.getValue(0, 0) as string);
    }
    return isExplainResult(result.columns, result.cells);
  }

  // Track the SQL statements for result tab labels
  let lastExecutedStatements = $state<string[]>([]);

  // ── Compare mode ──
  function enterCompareMode() {
    const nextIndex = tab.activeResultIndex + 1 < tab.queryResults.length
      ? tab.activeResultIndex + 1
      : 0;
    tab.compareMode = true;
    tab.compareConfig = {
      resultIndexA: tab.activeResultIndex,
      resultIndexB: nextIndex,
      matchMode: 'position',
    };
  }

  function updateCompareConfig(config: CompareConfig) {
    tab.compareConfig = config;
  }

  function exitCompareMode() {
    tab.compareMode = false;
    tab.compareConfig = undefined;
  }

  // ── Execution helpers ──
  function getEditorContent(): string {
    if (!editorView) return '';
    return editorView.state.doc.toString().trim();
  }

  function getSelectedText(): string {
    if (!editorView) return '';
    const sel = editorView.state.selection.main;
    if (sel.from === sel.to) return '';
    return editorView.state.sliceDoc(sel.from, sel.to).trim();
  }

  function executeCurrentQuery() {
    const selected = getSelectedText();
    const content = selected || getEditorContent();
    if (!content) return;
    lastExecutedStatements = splitStatements(content).map(s => s.text);
    forceRawView = false;
    app.executeQueryInTab(tab.id, content);
  }

  function executeCurrentStatement() {
    if (!editorView) return;
    const selected = getSelectedText();
    if (selected) {
      lastExecutedStatements = [selected];
      forceRawView = false;
      app.executeQueryInTab(tab.id, selected);
      return;
    }
    const fullText = editorView.state.doc.toString();
    const cursorPos = editorView.state.selection.main.head;
    const stmt = getStatementAtCursor(fullText, cursorPos);
    if (!stmt) return;
    lastExecutedStatements = [stmt];
    forceRawView = false;
    app.executeQueryInTab(tab.id, stmt);
  }

  function formatQuery() {
    if (!editorView) return;
    const lang = dialect.formatterLanguage();
    const selected = getSelectedText();
    if (selected) {
      const sel = editorView.state.selection.main;
      const formatted = formatSql(selected, lang);
      editorView.dispatch({
        changes: { from: sel.from, to: sel.to, insert: formatted },
      });
    } else {
      const content = getEditorContent();
      if (!content) return;
      const formatted = formatSql(content, lang);
      editorView.dispatch({
        changes: { from: 0, to: editorView.state.doc.length, insert: formatted },
      });
    }
  }

  function executeExplainAnalyze(json: boolean) {
    if (!editorView) return;
    const selected = getSelectedText();
    const content = selected || getEditorContent();
    if (!content) return;
    const wrapped = dialect.explainAnalyzeQuery(content, json);
    if (!wrapped) return;
    lastExecutedStatements = [wrapped];
    forceRawView = false;
    app.executeQueryInTab(tab.id, wrapped);
  }

  onMount(() => {
    const executeKeymap = Prec.highest(keymap.of([
      {
        key: 'Ctrl-Enter',
        mac: 'Cmd-Enter',
        run: () => {
          executeCurrentQuery();
          return true;
        },
      },
      {
        key: 'Ctrl-Shift-Enter',
        mac: 'Cmd-Shift-Enter',
        run: () => {
          executeCurrentStatement();
          return true;
        },
      },
      {
        key: 'Ctrl-Shift-f',
        mac: 'Cmd-Shift-f',
        run: () => {
          formatQuery();
          return true;
        },
      },
    ]));

    editorView = new EditorView({
      state: EditorState.create({
        doc: tab.content,
        extensions: [
          executeKeymap,
          basicSetup,
          sql({ dialect: dialect.codemirrorDialect() }),
          completionCompartment.of(
            autocompletion({ override: [completionSource] })
          ),
          oneDark,
          EditorView.theme({
            '&': { height: '100%', fontSize: '13px' },
            '.cm-scroller': { overflow: 'auto' },
            '.cm-content': { fontFamily: "'JetBrains Mono', 'Fira Code', monospace" },
          }),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) {
              app.updateQueryTabContent(tab.id, update.state.doc.toString());
            }
            if (update.selectionSet) {
              const sel = update.state.selection.main;
              hasSelection = sel.from !== sel.to;
            }
          }),
        ],
      }),
      parent: editorContainer,
    });

    return () => {
      editorView?.destroy();
    };
  });

  // Reload completion bundle when database or schema changes
  $effect(() => {
    const db = tab.databaseName;
    const schema = tab.schemaName;
    const connId = tab.savedConnectionId;

    if (!editorView || !db || !schema) return;

    let cancelled = false;
    app.getCompletionBundle(connId, db, schema).then((bundle) => {
      if (cancelled || !editorView) return;
      if (bundle) {
        currentBundle = bundle;
      }
    });

    return () => { cancelled = true; };
  });

  // Handle resize drag
  function startResize(e: MouseEvent) {
    e.preventDefault();
    resizing = true;
    const startY = e.clientY;
    const startHeight = editorHeight;

    function onMove(e: MouseEvent) {
      editorHeight = Math.max(80, Math.min(600, startHeight + (e.clientY - startY)));
    }

    function onUp() {
      resizing = false;
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    }

    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  // ── Elapsed timer ──
  let elapsedMs = $state(0);
  let timerInterval: ReturnType<typeof setInterval> | null = null;
  let startTime: number | null = null;

  $effect(() => {
    if (tab.isExecuting) {
      startTime = Date.now();
      elapsedMs = 0;
      timerInterval = setInterval(() => {
        elapsedMs = Date.now() - (startTime ?? Date.now());
      }, 100);
    } else {
      if (timerInterval) {
        clearInterval(timerInterval);
        timerInterval = null;
      }
      startTime = null;
      elapsedMs = 0;
    }

    return () => {
      if (timerInterval) {
        clearInterval(timerInterval);
        timerInterval = null;
      }
    };
  });

  function formatElapsed(ms: number): string {
    const totalSec = ms / 1000;
    if (totalSec < 60) {
      return `${totalSec.toFixed(1)}s`;
    }
    const min = Math.floor(totalSec / 60);
    const sec = Math.floor(totalSec % 60);
    return `${min}m ${sec}s`;
  }

  // ── Result tab keyboard nav ──
  function handleKeydown(e: KeyboardEvent) {
    if (tab.queryResults.length <= 1) return;
    if (e.altKey && e.key === 'ArrowLeft') {
      e.preventDefault();
      if (tab.activeResultIndex > 0) {
        app.setActiveResultIndex(tab.id, tab.activeResultIndex - 1);
      }
    } else if (e.altKey && e.key === 'ArrowRight') {
      e.preventDefault();
      if (tab.activeResultIndex < tab.queryResults.length - 1) {
        app.setActiveResultIndex(tab.id, tab.activeResultIndex + 1);
      }
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="flex flex-col h-full bg-background min-w-0">
  <!-- Query toolbar -->
  <QueryToolbar
    {tab}
    {elapsedMs}
    {formatElapsed}
    {hasSelection}
    onExecute={executeCurrentQuery}
    onExecuteStatement={executeCurrentStatement}
    onExplainAnalyze={() => executeExplainAnalyze(false)}
    onExplainAnalyzeJson={() => executeExplainAnalyze(true)}
    onFormat={formatQuery}
  />

  <!-- Editor area -->
  <div class="shrink-0 border-b border-border" style="height: {editorHeight}px;">
    <div class="h-full" bind:this={editorContainer}></div>
  </div>

  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <!-- Resize handle -->
  <div
    class="h-1 cursor-row-resize shrink-0 transition-colors {resizing ? 'bg-primary' : 'bg-border hover:bg-primary/50'}"
    onmousedown={startResize}
    role="separator"
  ></div>

  <!-- Results area -->
  <div class="flex-1 overflow-hidden flex flex-col">
    {#if tab.isExecuting && tab.queryResults.length === 0}
      <div class="flex-1 flex items-center justify-center">
        <div class="flex flex-col items-center gap-3">
          <div class="flex items-center gap-2 text-muted-foreground text-sm">
            <Loader2 class="h-4 w-4 animate-spin" />
            Executing query...
            <span class="tabular-nums text-warning font-medium">{formatElapsed(elapsedMs)}</span>
          </div>
        </div>
      </div>
    {:else if tab.queryResults.length > 0 && activeResult}
      <!-- Result tab bar (only shows for multi-result) -->
      <ResultTabBar
        results={tab.queryResults}
        activeIndex={tab.activeResultIndex}
        {totalExecutionTimeMs}
        statements={lastExecutedStatements}
        onselect={(index) => {
          app.setActiveResultIndex(tab.id, index);
          forceRawView = false;
        }}
      />

      <!-- Result content -->
      <div class="flex-1 overflow-hidden flex flex-col">
        {#if tab.compareMode && tab.compareConfig}
          <CompareView
            results={tab.queryResults}
            statements={lastExecutedStatements}
            config={tab.compareConfig}
            onupdate={updateCompareConfig}
            onclose={exitCompareMode}
          />
        {:else if isExplain}
          <ExplainViewer result={activeResult} onshowraw={() => { forceRawView = true; }} />
        {:else}
          <div class="flex-1 overflow-hidden">
            <DataGrid result={activeResult} />
          </div>
        {/if}

        <!-- Status bar (hidden in compare mode) -->
        {#if !tab.compareMode}
          <QueryResultBar
            result={activeResult}
            resultIndex={tab.activeResultIndex}
            totalResults={tab.queryResults.length}
            {totalExecutionTimeMs}
            oncompare={tab.queryResults.length > 1 ? enterCompareMode : undefined}
          />
        {/if}
      </div>
    {:else}
      <div class="flex items-center justify-center h-full text-muted-foreground text-sm">
        Run a query to see results
      </div>
    {/if}
  </div>
</div>
