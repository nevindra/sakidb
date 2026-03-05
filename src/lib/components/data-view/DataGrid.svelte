<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { getAppState } from '$lib/stores';
  import type { QueryResult, CellValue, ColumnInfo, TableFilter, AnyQueryResult } from '$lib/types';
  import { ColumnarResultData } from '$lib/types/query-result-data';
  import CellDisplay from './CellDisplay.svelte';
  import CellEditor from './CellEditor.svelte';
  import CellExpandPopover from './CellExpandPopover.svelte';
  import RowDetailPanel from './RowDetailPanel.svelte';
  import GridFilterBar from './GridFilterBar.svelte';
  import GridContextMenu from './GridContextMenu.svelte';
  import GridBottomBar from './GridBottomBar.svelte';
  import ConfirmDialog from '../ui/confirm-dialog/ConfirmDialog.svelte';
  import ExportDialog from '../structure/ExportDialog.svelte';
  import { Plus, Trash2, Undo2, Download } from '@lucide/svelte';
  import {
    generateUpdateSql,
    generateInsertSql,
    generateDeleteSql,
    wrapInTransaction,
    getPkColumnIndices,
    cellValueEquals,
  } from '$lib/sql-utils';

  let {
    result,
    tabId = '',
    class: className = '',
    schema = '',
    table = '',
    connectionId = '',
    savedConnectionId = '',
    databaseName = '',
    columnInfos = [] as ColumnInfo[],
    filters = [] as TableFilter[],
    currentPage = 0,
    pageSize = 50,
    totalRowEstimate = 0,
    onreload,
  }: {
    result: AnyQueryResult;
    tabId?: string;
    class?: string;
    schema?: string;
    table?: string;
    connectionId?: string;
    savedConnectionId?: string;
    databaseName?: string;
    columnInfos?: ColumnInfo[];
    filters?: TableFilter[];
    currentPage?: number;
    pageSize?: number;
    totalRowEstimate?: number;
    onreload?: () => void;
  } = $props();

  let scrollContainer = $state<HTMLDivElement | null>(null);
  let gridElement = $state<HTMLDivElement | null>(null);
  let sortCol = $state<number | null>(null);
  let sortAsc = $state(true);

  const ROW_HEIGHT = 28;
  const OVERSCAN = 10;
  const DEFAULT_COL_WIDTH = 150;
  const MIN_COL_WIDTH = 60;
  const MAX_COL_WIDTH = 400;
  const COL_WIDTH_NUM = 48;
  const CHAR_WIDTH = 7.5; // approx px per char at text-xs
  const COL_PADDING = 24; // px horizontal padding in cell
  const KEY_STRIDE = 100_000; // supports up to 100K columns
  function cellKey(dataRow: number, colIdx: number): number {
    return dataRow * KEY_STRIDE + colIdx;
  }

  const isColumnar = $derived(result instanceof ColumnarResultData);

  let scrollTop = $state(0);
  let containerHeight = $state(400);

  // ── Column resize state ──
  let colWidths = $state<number[]>([]);
  let resizing = $state<{ colIndex: number; startX: number; startWidth: number } | null>(null);

  function getCellTextLength(row: number, colIdx: number): number {
    if (result instanceof ColumnarResultData) {
      const cd = result.columnData[colIdx];
      if (cd.nulls[row] !== 0) return 4;
      switch (cd.type) {
        case 'number': return String(cd.values[row]).length;
        case 'bool': return cd.values[row] ? 4 : 5;
        case 'text': return result.getTextByteLength(row, colIdx); // byte length ≈ char length, avoids decode
        case 'bytes': return 8;
      }
    }
    // Legacy path
    const cell = (result as QueryResult).cells[row * numCols + colIdx];
    if (cell === 'Null') return 4;
    if ('Bool' in cell) return cell.Bool ? 4 : 5;
    if ('Int' in cell) return String(cell.Int).length;
    if ('Float' in cell) return String(cell.Float).length;
    if ('Text' in cell) return cell.Text.length;
    if ('Json' in cell) return Math.min(cell.Json.length, 40);
    if ('Timestamp' in cell) return cell.Timestamp.length;
    if ('Bytes' in cell) return 8;
    return 4;
  }

  function estimateColumnWidths(): number[] {
    const cols = result.columns;
    const nc = cols.length;
    const rc = result instanceof ColumnarResultData
      ? result.row_count
      : (nc > 0 ? Math.floor((result as QueryResult).cells.length / nc) : 0);
    const sampleSize = Math.min(rc, 100);

    return cols.map((col, colIdx) => {
      const headerLen = Math.max(col.name.length, col.data_type.length);

      let maxDataLen = 0;
      for (let r = 0; r < sampleSize; r++) {
        const len = getCellTextLength(r, colIdx);
        if (len > maxDataLen) maxDataLen = len;
      }

      const contentLen = Math.max(headerLen, maxDataLen);
      const width = contentLen * CHAR_WIDTH + COL_PADDING;
      return Math.max(MIN_COL_WIDTH, Math.min(MAX_COL_WIDTH, Math.round(width)));
    });
  }

  // Initialize column widths when columns change
  $effect(() => {
    const cols = result.columns;
    if (cols.length !== colWidths.length) {
      colWidths = estimateColumnWidths();
    }
  });

  const totalTableWidth = $derived(
    COL_WIDTH_NUM + colWidths.reduce((sum, w) => sum + w, 0)
  );

  function handleResizeStart(e: MouseEvent, colIndex: number) {
    e.preventDefault();
    e.stopPropagation();
    resizing = { colIndex, startX: e.clientX, startWidth: colWidths[colIndex] };

    let rafPending = false;
    function onMouseMove(ev: MouseEvent) {
      if (!resizing || rafPending) return;
      rafPending = true;
      requestAnimationFrame(() => {
        rafPending = false;
        if (!resizing) return;
        const delta = ev.clientX - resizing.startX;
        colWidths[resizing.colIndex] = Math.max(MIN_COL_WIDTH, resizing.startWidth + delta);
        colWidths = colWidths;
      });
    }

    function onMouseUp() {
      resizing = null;
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
    }

    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  }

  // Context menu state
  let contextMenu = $state<{ x: number; y: number; rowIndex: number; colIndex: number } | null>(null);

  // Cell expand popover state
  let expandedCell = $state<{ rowIndex: number; colIndex: number; rect: DOMRect } | null>(null);

  // Row detail panel state
  let selectedRowIndex = $state<number | null>(null);
  let detailPanelOpen = $state(false);

  // ── Edit mode state ──
  let editMode = $state(false);
  let pendingUpdates = $state(new Map<number, CellValue>()); // cellKey(dataRow, colIdx) → newValue
  let pendingInserts = $state<CellValue[][]>([]); // each is a full row
  let pendingDeletes = $state(new Set<number>()); // dataRowIndex
  let focusedCell = $state<{ row: number; col: number } | null>(null);
  let editingCell = $state<{ row: number; col: number } | null>(null);
  let selectedRows = $state(new Set<number>()); // displayRowIndex for bulk ops
  let isApplying = $state(false);
  let confirmApplyOpen = $state(false);
  let confirmDiscardOpen = $state(false);
  let exportOpen = $state(false);

  // Undo stack
  type UndoAction =
    | { type: 'update'; key: number; oldValue: CellValue | undefined }
    | { type: 'insert' }
    | { type: 'delete'; dataRow: number };
  let undoStack = $state<UndoAction[]>([]);

  // ── Derived values ──
  const isDataTab = $derived(tabId !== '');
  const numCols = $derived(result.columns.length);
  const rowCount = $derived(
    result instanceof ColumnarResultData
      ? result.row_count
      : (numCols > 0 ? Math.floor((result as QueryResult).cells.length / numCols) : 0)
  );

  const pkColIndices = $derived(getPkColumnIndices(result.columns, columnInfos));
  const pkColumnNames = $derived(pkColIndices.map(i => result.columns[i].name));
  const canEdit = $derived(pkColIndices.length > 0 && schema !== '' && table !== '' && connectionId !== '');

  const totalDisplayRows = $derived(rowCount + pendingInserts.length);
  const totalChanges = $derived.by(() => {
    const rows = new Set<number>();
    for (const key of pendingUpdates.keys()) rows.add(Math.floor(key / KEY_STRIDE));
    return rows.size + pendingInserts.length + pendingDeletes.size;
  });
  const updateCount = $derived.by(() => {
    const rows = new Set<number>();
    for (const key of pendingUpdates.keys()) rows.add(Math.floor(key / KEY_STRIDE));
    return rows.size;
  });
  const insertCount = $derived(pendingInserts.length);
  const deleteCount = $derived(pendingDeletes.size);
  const canExport = $derived(savedConnectionId !== '' && databaseName !== '' && schema !== '' && table !== '');
  const app = getAppState();
  const filterWhereClause = $derived.by(() => {
    if (filters.length === 0) return undefined;
    const clauses = filters.map(f => app.filterToSql(f)).filter(Boolean);
    return clauses.length > 0 ? clauses.join(' AND ') : undefined;
  });

  // Pagination
  const totalPages = $derived(
    totalRowEstimate > 0
      ? Math.max(1, Math.ceil(totalRowEstimate / pageSize))
      : Math.max(1, Math.ceil(rowCount / pageSize))
  );

  // ── Sort indices ──
  const sortIndices = $derived.by(() => {
    if (sortCol === null) return null;
    const count = rowCount;
    const col = sortCol;
    const asc = sortAsc;

    // Pre-extract sort keys in O(n)
    const keys: (string | number)[] = new Array(count);
    if (result instanceof ColumnarResultData) {
      // Fast path: direct typed array access
      for (let i = 0; i < count; i++) {
        keys[i] = result.getSortKey(i, col);
      }
    } else {
      // Legacy path
      const nc = numCols;
      const cells = result.cells;
      for (let i = 0; i < count; i++) {
        keys[i] = getCellSortValue(cells[i * nc + col]);
      }
    }

    const indices = new Uint32Array(count);
    for (let i = 0; i < count; i++) indices[i] = i;

    indices.sort((a, b) => {
      const av = keys[a];
      const bv = keys[b];
      if (av < bv) return asc ? -1 : 1;
      if (av > bv) return asc ? 1 : -1;
      return 0;
    });
    return indices;
  });

  // ── Visible range ──
  const visibleRange = $derived.by(() => {
    const start = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
    const visibleCount = Math.ceil(containerHeight / ROW_HEIGHT) + OVERSCAN * 2;
    const end = Math.min(totalDisplayRows, start + visibleCount);
    return { start, end };
  });

  // ── Cell access helpers ──
  function getDataRowIndex(displayIndex: number): number {
    if (displayIndex >= rowCount) return -1;
    return sortIndices ? sortIndices[displayIndex] : displayIndex;
  }

  function getOriginalCell(dataRow: number, colIndex: number): CellValue {
    if (result instanceof ColumnarResultData) {
      return result.toCellValue(dataRow, colIndex);
    }
    return result.cells[dataRow * numCols + colIndex];
  }

  function getCell(displayIdx: number, colIdx: number): CellValue {
    const dataRow = getDataRowIndex(displayIdx);
    if (result instanceof ColumnarResultData) {
      return result.toCellValue(dataRow, colIdx);
    }
    return result.cells[dataRow * numCols + colIdx];
  }

  function getDisplayCell(displayIdx: number, colIdx: number): CellValue {
    if (isInsertRow(displayIdx)) {
      const insertIdx = displayIdx - rowCount;
      return pendingInserts[insertIdx]?.[colIdx] ?? 'Null';
    }
    const dataRow = getDataRowIndex(displayIdx);
    if (pendingUpdates.size > 0) {
      const pending = pendingUpdates.get(cellKey(dataRow, colIdx));
      if (pending !== undefined) return pending;
    }
    if (result instanceof ColumnarResultData) {
      return result.toCellValue(dataRow, colIdx);
    }
    return result.cells[dataRow * numCols + colIdx];
  }

  function getRowCells(displayRowIndex: number): CellValue[] {
    const cells: CellValue[] = [];
    for (let i = 0; i < numCols; i++) {
      cells.push(getDisplayCell(displayRowIndex, i));
    }
    return cells;
  }

  function isInsertRow(displayIdx: number): boolean {
    return displayIdx >= rowCount;
  }

  function isDeletedRow(displayIdx: number): boolean {
    if (isInsertRow(displayIdx)) return false;
    return pendingDeletes.has(getDataRowIndex(displayIdx));
  }

  function isCellModified(displayIdx: number, colIdx: number): boolean {
    if (isInsertRow(displayIdx)) return true;
    if (pendingUpdates.size === 0) return false;
    const dataRow = getDataRowIndex(displayIdx);
    return pendingUpdates.has(cellKey(dataRow, colIdx));
  }

  // ── Sort helpers ──
  function handleSort(colIdx: number) {
    if (sortCol === colIdx) {
      sortAsc = !sortAsc;
    } else {
      sortCol = colIdx;
      sortAsc = true;
    }
  }

  function getCellSortValue(cell: CellValue): string | number {
    if (cell === 'Null') return '';
    if ('Bool' in cell) return cell.Bool ? 1 : 0;
    if ('Int' in cell) return cell.Int;
    if ('Float' in cell) return cell.Float;
    if ('Text' in cell) return cell.Text;
    if ('Json' in cell) return cell.Json;
    if ('Timestamp' in cell) return cell.Timestamp;
    if ('Bytes' in cell) return '';
    return '';
  }

  // ── Row detail panel ──
  const detailRow = $derived(
    selectedRowIndex !== null && selectedRowIndex < totalDisplayRows
      ? getRowCells(selectedRowIndex)
      : null
  );

  // ── Scroll handling ──
  let rafId: number | null = null;

  function handleScroll() {
    if (expandedCell) expandedCell = null;
    if (contextMenu) contextMenu = null;
    if (rafId !== null) return;
    rafId = requestAnimationFrame(() => {
      rafId = null;
      if (scrollContainer) {
        scrollTop = scrollContainer.scrollTop;
      }
    });
  }

  function observeResize(node: HTMLDivElement) {
    const observer = new ResizeObserver((entries) => {
      containerHeight = entries[0].contentRect.height;
    });
    observer.observe(node);
    return { destroy() { observer.disconnect(); } };
  }

  // ── Cell interaction ──
  function handleCellClick(e: MouseEvent, rowIndex: number, colIndex: number) {
    if (editMode) {
      e.stopPropagation();
      focusedCell = { row: rowIndex, col: colIndex };
      expandedCell = null;
      contextMenu = null;
      if (editingCell?.row === rowIndex && editingCell?.col === colIndex) return;
      editingCell = null;
      return;
    }

    if (expandedCell && expandedCell.rowIndex === rowIndex && expandedCell.colIndex === colIndex) {
      expandedCell = null;
      return;
    }
    const td = e.currentTarget as HTMLElement;
    const rect = td.getBoundingClientRect();
    expandedCell = { rowIndex, colIndex, rect };
    contextMenu = null;
  }

  function handleCellDblClick(e: MouseEvent, rowIndex: number, colIndex: number) {
    if (!canEdit) return;
    e.stopPropagation();
    if (!editMode) editMode = true;
    if (isDeletedRow(rowIndex)) return;
    const dataType = result.columns[colIndex]?.data_type ?? '';
    if (dataType.toLowerCase() === 'bytea') return;
    focusedCell = { row: rowIndex, col: colIndex };
    editingCell = { row: rowIndex, col: colIndex };
    expandedCell = null;
  }

  function handleRowDblClick(rowIndex: number) {
    selectedRowIndex = rowIndex;
    detailPanelOpen = true;
    expandedCell = null;
    contextMenu = null;
  }

  // ── Row number click (selection) ──
  function handleRowNumClick(e: MouseEvent, displayIdx: number) {
    if (!canEdit) return;
    e.stopPropagation();
    if (!editMode) editMode = true;

    if (e.ctrlKey || e.metaKey) {
      const next = new Set(selectedRows);
      if (next.has(displayIdx)) next.delete(displayIdx);
      else next.add(displayIdx);
      selectedRows = next;
    } else if (e.shiftKey && selectedRows.size > 0) {
      const existing = [...selectedRows];
      const anchor = existing[existing.length - 1];
      const start = Math.min(anchor, displayIdx);
      const end = Math.max(anchor, displayIdx);
      const next = new Set(selectedRows);
      for (let i = start; i <= end; i++) next.add(i);
      selectedRows = next;
    } else {
      selectedRows = new Set([displayIdx]);
    }
  }

  // ── Context menu handlers ──
  function handleContextMenu(e: MouseEvent, rowIndex: number, colIndex: number) {
    e.preventDefault();
    contextMenu = { x: e.clientX, y: e.clientY, rowIndex, colIndex };
    expandedCell = null;
  }

  function ctxEditCell() {
    if (!contextMenu) return;
    const { rowIndex, colIndex } = contextMenu;
    if (!editMode) editMode = true;
    if (isDeletedRow(rowIndex)) return;
    const dataType = result.columns[colIndex]?.data_type ?? '';
    if (dataType.toLowerCase() === 'bytea') return;
    focusedCell = { row: rowIndex, col: colIndex };
    editingCell = { row: rowIndex, col: colIndex };
    expandedCell = null;
  }

  function ctxInsertRow() {
    if (!editMode) editMode = true;
    addRow();
  }

  function ctxDeleteRow() {
    if (!contextMenu) return;
    const { rowIndex } = contextMenu;
    if (!editMode) editMode = true;

    if (isInsertRow(rowIndex)) {
      const insertIdx = rowIndex - rowCount;
      pendingInserts = pendingInserts.filter((_, i) => i !== insertIdx);
    } else {
      const dataRow = getDataRowIndex(rowIndex);
      pendingDeletes.add(dataRow);
      pendingDeletes = new Set(pendingDeletes);
      undoStack = [...undoStack, { type: 'delete', dataRow }];
    }
  }

  function ctxViewDetails() {
    if (!contextMenu) return;
    selectedRowIndex = contextMenu.rowIndex;
    detailPanelOpen = true;
  }

  function handleDetailNavigate(direction: 'prev' | 'next') {
    if (selectedRowIndex === null) return;
    if (direction === 'prev' && selectedRowIndex > 0) {
      selectedRowIndex = selectedRowIndex - 1;
    } else if (direction === 'next' && selectedRowIndex < totalDisplayRows - 1) {
      selectedRowIndex = selectedRowIndex + 1;
    }
  }

  function getContainerRect(): DOMRect {
    return scrollContainer?.getBoundingClientRect() ?? new DOMRect();
  }

  // ── Edit operations ──
  function handleCellConfirm(displayIdx: number, colIdx: number, newValue: CellValue) {
    if (isInsertRow(displayIdx)) {
      const insertIdx = displayIdx - rowCount;
      const row = [...pendingInserts[insertIdx]];
      row[colIdx] = newValue;
      pendingInserts[insertIdx] = row;
      pendingInserts = [...pendingInserts];
      undoStack = [...undoStack, { type: 'insert' }];
    } else {
      const dataRow = getDataRowIndex(displayIdx);
      const key = cellKey(dataRow, colIdx);
      const oldValue = pendingUpdates.get(key);
      const original = getOriginalCell(dataRow, colIdx);
      if (cellValueEquals(newValue, original)) {
        pendingUpdates.delete(key);
      } else {
        pendingUpdates.set(key, newValue);
      }
      pendingUpdates = new Map(pendingUpdates);
      undoStack = [...undoStack, { type: 'update', key, oldValue }];
    }
    editingCell = null;
  }

  function handleCellCancel() {
    editingCell = null;
  }

  function handleCellTab(displayIdx: number, colIdx: number, shift: boolean) {
    editingCell = null;
    const nextCol = shift ? colIdx - 1 : colIdx + 1;
    if (nextCol >= 0 && nextCol < numCols) {
      focusedCell = { row: displayIdx, col: nextCol };
      if (!isDeletedRow(displayIdx)) {
        const dataType = result.columns[nextCol]?.data_type ?? '';
        if (dataType.toLowerCase() !== 'bytea') {
          editingCell = { row: displayIdx, col: nextCol };
        }
      }
    }
  }

  function addRow() {
    if (!editMode) editMode = true;
    const newRow: CellValue[] = new Array(numCols).fill('Null');
    pendingInserts = [...pendingInserts, newRow];
    undoStack = [...undoStack, { type: 'insert' }];
    requestAnimationFrame(() => {
      if (scrollContainer) {
        scrollContainer.scrollTop = (rowCount + pendingInserts.length - 1) * ROW_HEIGHT;
      }
      focusedCell = { row: rowCount + pendingInserts.length - 1, col: 0 };
      const dataType = result.columns[0]?.data_type ?? '';
      if (dataType.toLowerCase() !== 'bytea') {
        editingCell = { row: rowCount + pendingInserts.length - 1, col: 0 };
      }
    });
  }

  function deleteSelectedRows() {
    for (const displayIdx of selectedRows) {
      if (isInsertRow(displayIdx)) {
        const insertIdx = displayIdx - rowCount;
        pendingInserts = pendingInserts.filter((_, i) => i !== insertIdx);
      } else {
        const dataRow = getDataRowIndex(displayIdx);
        pendingDeletes.add(dataRow);
        pendingDeletes = new Set(pendingDeletes);
        undoStack = [...undoStack, { type: 'delete', dataRow }];
      }
    }
    selectedRows = new Set();
  }

  function handleUndo() {
    if (undoStack.length === 0) return;
    const action = undoStack[undoStack.length - 1];
    undoStack = undoStack.slice(0, -1);

    if (action.type === 'update') {
      if (action.oldValue !== undefined) {
        pendingUpdates.set(action.key, action.oldValue);
      } else {
        pendingUpdates.delete(action.key);
      }
      pendingUpdates = new Map(pendingUpdates);
    } else if (action.type === 'insert') {
      if (pendingInserts.length > 0) {
        pendingInserts = pendingInserts.slice(0, -1);
      }
    } else if (action.type === 'delete') {
      pendingDeletes.delete(action.dataRow);
      pendingDeletes = new Set(pendingDeletes);
    }
  }

  function discardChanges() {
    pendingUpdates = new Map();
    pendingInserts = [];
    pendingDeletes = new Set();
    undoStack = [];
    editingCell = null;
    focusedCell = null;
    selectedRows = new Set();
  }

  // ── Apply changes ──
  async function applyChanges() {
    if (!canEdit) return;
    isApplying = true;

    try {
      const statements: string[] = [];

      const pkDataTypes = pkColIndices.map(i => result.columns[i].data_type);

      for (const dataRow of pendingDeletes) {
        const pkValues = pkColIndices.map(i => getOriginalCell(dataRow, i));
        statements.push(generateDeleteSql(schema, table, pkColumnNames, pkValues, pkDataTypes));
      }

      const updatesByRow = new Map<number, [string, CellValue, string?][]>();
      for (const [key, val] of pendingUpdates) {
        const dataRow = Math.floor(key / KEY_STRIDE);
        const colIdx = key % KEY_STRIDE;
        if (!updatesByRow.has(dataRow)) updatesByRow.set(dataRow, []);
        updatesByRow.get(dataRow)!.push([result.columns[colIdx].name, val, result.columns[colIdx].data_type]);
      }
      for (const [dataRow, changes] of updatesByRow) {
        const pkValues = pkColIndices.map(i => getOriginalCell(dataRow, i));
        statements.push(generateUpdateSql(schema, table, pkColumnNames, pkValues, changes, pkDataTypes));
      }

      for (const insertRow of pendingInserts) {
        const colNames = result.columns.map(c => c.name);
        const colDataTypes = result.columns.map(c => c.data_type);
        statements.push(generateInsertSql(schema, table, colNames, insertRow, colDataTypes));
      }

      if (statements.length === 0) return;

      const sql = wrapInTransaction(statements);
      await invoke('execute_batch', {
        activeConnectionId: connectionId,
        sql,
      });

      discardChanges();
      onreload?.();
    } catch (e) {
      alert(`Apply failed: ${e}`);
    } finally {
      isApplying = false;
      confirmApplyOpen = false;
    }
  }

  // ── Keyboard navigation ──
  function handleWindowClick(e: MouseEvent) {
    contextMenu = null;
    if (editMode && gridElement && !gridElement.contains(e.target as Node)) {
      if (!editingCell) {
        focusedCell = null;
        selectedRows = new Set();
      }
    }
  }

  function handleWindowKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (editingCell) {
        editingCell = null;
        return;
      }
      if (expandedCell) {
        expandedCell = null;
        return;
      }
      contextMenu = null;
      return;
    }

    if (!editMode || editingCell || !focusedCell) return;

    if (e.key === 'ArrowUp') {
      e.preventDefault();
      focusedCell = { row: Math.max(0, focusedCell.row - 1), col: focusedCell.col };
      scrollToRow(focusedCell.row);
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      focusedCell = { row: Math.min(totalDisplayRows - 1, focusedCell.row + 1), col: focusedCell.col };
      scrollToRow(focusedCell.row);
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      focusedCell = { row: focusedCell.row, col: Math.max(0, focusedCell.col - 1) };
    } else if (e.key === 'ArrowRight') {
      e.preventDefault();
      focusedCell = { row: focusedCell.row, col: Math.min(numCols - 1, focusedCell.col + 1) };
    } else if (e.key === 'Enter' || e.key === 'F2') {
      e.preventDefault();
      if (!isDeletedRow(focusedCell.row)) {
        const dataType = result.columns[focusedCell.col]?.data_type ?? '';
        if (dataType.toLowerCase() !== 'bytea') {
          editingCell = { ...focusedCell };
        }
      }
    } else if (e.key === 'Delete' || e.key === 'Backspace') {
      if (selectedRows.size > 0) {
        e.preventDefault();
        deleteSelectedRows();
      }
    } else if (e.key === 'z' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleUndo();
    } else if (e.key === 'Tab') {
      e.preventDefault();
      const nextCol = e.shiftKey ? focusedCell.col - 1 : focusedCell.col + 1;
      if (nextCol >= 0 && nextCol < numCols) {
        focusedCell = { row: focusedCell.row, col: nextCol };
      }
    }
  }

  function scrollToRow(rowIdx: number) {
    if (!scrollContainer) return;
    const rowTop = rowIdx * ROW_HEIGHT;
    const rowBottom = rowTop + ROW_HEIGHT;
    const viewTop = scrollContainer.scrollTop;
    const viewBottom = viewTop + containerHeight;

    if (rowTop < viewTop + ROW_HEIGHT) {
      scrollContainer.scrollTop = Math.max(0, rowTop - ROW_HEIGHT);
    } else if (rowBottom > viewBottom) {
      scrollContainer.scrollTop = rowBottom - containerHeight;
    }
  }
</script>

<svelte:window onclick={handleWindowClick} onkeydown={handleWindowKeydown} />

<div class="flex flex-col h-full overflow-hidden min-w-0 {className}" bind:this={gridElement}>
  {#if result.columns.length > 0 || isDataTab}
    <!-- ═══ Top toolbar: filter + edit actions (DataTab only) ═══ -->
    {#if isDataTab}
    <div class="border-b border-border bg-card shrink-0">
      <div class="flex items-center gap-1.5 px-2 py-1 min-h-[28px]">
        <!-- Filter section -->
        <GridFilterBar columns={result.columns} {filters} {tabId} onrefresh={onreload} />

        <!-- Export button -->
        {#if canExport}
          <div class="w-px h-4 bg-border/60 shrink-0"></div>
          <button
            class="p-1 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors shrink-0"
            onclick={() => (exportOpen = true)}
            title="Export table"
          >
            <Download class="h-3.5 w-3.5" />
          </button>
        {/if}

        <!-- Edit actions -->
        {#if canEdit}
          <div class="w-px h-4 bg-border/60 shrink-0"></div>

          <button
            class="p-1 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors shrink-0"
            onclick={addRow}
            disabled={isApplying}
            title="Add row"
          >
            <Plus class="h-3.5 w-3.5" />
          </button>

          {#if selectedRows.size > 0}
            <button
              class="p-1 rounded text-destructive hover:bg-destructive/10 transition-colors shrink-0"
              onclick={deleteSelectedRows}
              disabled={isApplying}
              title="Delete selected rows"
            >
              <Trash2 class="h-3.5 w-3.5" />
            </button>
          {/if}

          {#if totalChanges > 0}
            <div class="w-px h-4 bg-border/60 shrink-0"></div>

            <button
              class="p-1 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors disabled:opacity-30 shrink-0"
              onclick={handleUndo}
              disabled={isApplying}
              title="Undo (Ctrl+Z)"
            >
              <Undo2 class="h-3.5 w-3.5" />
            </button>

            <span class="text-[11px] text-muted-foreground tabular-nums shrink-0">
              {totalChanges}
            </span>

            <button
              class="text-[11px] px-1.5 py-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors shrink-0"
              onclick={() => { confirmDiscardOpen = true; }}
              disabled={isApplying}
            >
              Discard
            </button>

            <button
              class="text-[11px] px-2 py-0.5 rounded bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 shrink-0"
              onclick={() => { confirmApplyOpen = true; }}
              disabled={isApplying}
            >
              {isApplying ? 'Applying...' : 'Apply'}
            </button>
          {/if}
        {/if}
      </div>
    </div>
    {/if}

    <!-- ═══ Table scroll area ═══ -->
    <div class="flex-1 min-h-0 relative">
      <div
        class="absolute inset-0 overflow-auto"
        bind:this={scrollContainer}
        onscroll={handleScroll}
        use:observeResize
      >
        <table class="border-collapse text-xs" style="table-layout: fixed; min-width: 100%; width: {totalTableWidth}px;">
          <thead class="sticky top-0 z-10">
            <tr class="bg-card">
              <th class="px-2 py-1.5 text-left font-medium text-text-dim border-b border-border sticky left-0 z-20 bg-card after:content-[''] after:absolute after:top-0 after:right-0 after:bottom-0 after:w-px after:bg-border" style="width: {COL_WIDTH_NUM}px;">#</th>
              {#each result.columns as col, i}
                <th
                  class="relative px-2 py-1.5 text-left font-medium text-muted-foreground border-b border-border cursor-pointer hover:text-foreground hover:bg-accent/70 select-none overflow-hidden text-ellipsis whitespace-nowrap group/th transition-colors"
                  style="width: {colWidths[i] ?? DEFAULT_COL_WIDTH}px;"
                  onclick={() => handleSort(i)}
                >
                  <span>{col.name}</span>
                  {#if sortCol === i}
                    <span class="ml-1 text-primary">{sortAsc ? '↑' : '↓'}</span>
                  {/if}
                  <span class="block text-text-dim font-normal text-[10px]">{col.data_type}</span>
                  <!-- Resize handle -->
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div
                    class="absolute top-0 right-0 w-1.5 h-full cursor-col-resize opacity-0 group-hover/th:opacity-100 hover:!opacity-100 z-10
                      {resizing?.colIndex === i ? '!opacity-100 bg-primary/60' : 'hover:bg-primary/40'}"
                    onmousedown={(e) => handleResizeStart(e, i)}
                  ></div>
                </th>
              {/each}
            </tr>
          </thead>
          <tbody>
            <tr style="height: {visibleRange.start * ROW_HEIGHT}px;">
              <td colspan={result.columns.length + 1}></td>
            </tr>

            {#each { length: visibleRange.end - visibleRange.start } as _, i}
              {@const displayIdx = visibleRange.start + i}
              {@const isInsert = isInsertRow(displayIdx)}
              {@const isDeleted = isDeletedRow(displayIdx)}
              {@const isSelected = selectedRows.has(displayIdx)}
              <tr
                class="transition-colors
                  {isDeleted ? 'opacity-40 line-through' : ''}
                  {isSelected ? 'bg-primary/10' : ''}
                  {selectedRowIndex === displayIdx && !editMode ? 'bg-accent/20' : ''}
                  {!isDeleted && !isSelected && displayIdx % 2 === 1 ? 'bg-muted/30' : ''}
                  {!isDeleted && !isSelected ? 'hover:bg-accent/30' : ''}"
                style="height: {ROW_HEIGHT}px;"
              >
                <td
                  class="px-2 tabular-nums sticky left-0 relative after:content-[''] after:absolute after:top-0 after:right-0 after:bottom-0 after:w-px after:bg-border
                    {canEdit ? 'cursor-pointer select-none' : ''}
                    {isInsert ? 'text-success bg-success/5' : isDeleted ? 'text-destructive bg-destructive/5' : displayIdx % 2 === 1 ? 'text-text-dim bg-muted/30' : 'text-text-dim bg-card'}
                    {isSelected ? '!bg-primary/10' : ''}
                    {selectedRowIndex === displayIdx && !editMode ? '!bg-accent/20' : ''}"
                  style="width: {COL_WIDTH_NUM}px; contain: strict;"
                  onclick={(e) => handleRowNumClick(e, displayIdx)}
                >
                  {#if isInsert}
                    +
                  {:else}
                    {displayIdx + 1}
                  {/if}
                </td>

                {#each result.columns as col, colIdx}
                  {@const isFocused = editMode && focusedCell?.row === displayIdx && focusedCell?.col === colIdx}
                  {@const isEditing = editingCell?.row === displayIdx && editingCell?.col === colIdx}
                  {@const isModified = editMode && isCellModified(displayIdx, colIdx)}
                  <td
                    class="overflow-hidden text-ellipsis whitespace-nowrap
                      {editMode ? 'cursor-cell' : 'cursor-pointer'}
                      {isModified ? 'border-l-2 border-l-warning' : ''}
                      {isInsert && !isModified ? 'border-l-2 border-l-success/30' : ''}
                      {isFocused && !isEditing ? 'ring-1 ring-inset ring-primary/60' : ''}
                      {!isEditing ? 'px-2' : ''}
                      {!isDeleted ? 'hover:bg-accent/20' : ''}"
                    style="contain: {isEditing ? 'none' : 'content'};"
                    onclick={(e) => handleCellClick(e, displayIdx, colIdx)}
                    ondblclick={(e) => handleCellDblClick(e, displayIdx, colIdx)}
                    oncontextmenu={(e) => handleContextMenu(e, displayIdx, colIdx)}
                  >
                    {#if isEditing && !isDeleted}
                      <CellEditor
                        value={getDisplayCell(displayIdx, colIdx)}
                        dataType={col.data_type}
                        onconfirm={(v) => handleCellConfirm(displayIdx, colIdx, v)}
                        oncancel={handleCellCancel}
                        ontab={(shift) => handleCellTab(displayIdx, colIdx, shift)}
                      />
                    {:else if isColumnar && !isInsert && pendingUpdates.size === 0}
                      {@const dataRow = getDataRowIndex(displayIdx)}
                      {@const display = (result as ColumnarResultData).getCellDisplay(dataRow, colIdx)}
                      <span class={display.cls}>{display.text}</span>
                    {:else}
                      <CellDisplay value={getDisplayCell(displayIdx, colIdx)} dataType={col.data_type} />
                    {/if}
                  </td>
                {/each}
              </tr>
            {/each}

            <tr style="height: {Math.max(0, (totalDisplayRows - visibleRange.end)) * ROW_HEIGHT}px;">
              <td colspan={result.columns.length + 1}></td>
            </tr>
          </tbody>
        </table>

        {#if !editMode && expandedCell && expandedCell.rowIndex < rowCount && expandedCell.colIndex < numCols}
          <CellExpandPopover
            cell={getCell(expandedCell.rowIndex, expandedCell.colIndex)}
            column={result.columns[expandedCell.colIndex]}
            anchorRect={expandedCell.rect}
            containerRect={getContainerRect()}
            onclose={() => (expandedCell = null)}
          />
        {/if}
      </div>
    </div>

    <!-- ═══ Bottom bar (DataTab only) ═══ -->
    {#if isDataTab}
      <GridBottomBar
        result={result as QueryResult}
        {tabId}
        {currentPage}
        {pageSize}
        {totalPages}
        {canEdit}
        {schema}
        pendingInsertCount={insertCount}
        pendingDeleteCount={deleteCount}
      />
    {/if}
  {:else}
    <div class="flex-1 flex items-center justify-center text-muted-foreground text-sm">
      <div class="text-center space-y-1">
        <p>Query executed successfully</p>
        <p class="text-xs">{result.row_count} rows affected in {result.execution_time_ms}ms</p>
      </div>
    </div>
  {/if}
</div>

<!-- Context menu -->
{#if contextMenu}
  <GridContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    cell={getDisplayCell(contextMenu.rowIndex, contextMenu.colIndex)}
    row={getRowCells(contextMenu.rowIndex)}
    columns={result.columns}
    columnName={result.columns[contextMenu.colIndex]?.name ?? ''}
    {canEdit}
    onclose={() => { contextMenu = null; }}
    oneditcell={ctxEditCell}
    oninsertrow={ctxInsertRow}
    ondeleterow={ctxDeleteRow}
    onviewdetails={ctxViewDetails}
  />
{/if}

<!-- Row detail panel -->
<RowDetailPanel
  bind:open={detailPanelOpen}
  row={detailRow}
  rowIndex={selectedRowIndex ?? 0}
  columns={result.columns}
  totalRows={totalDisplayRows}
  onnavigate={handleDetailNavigate}
/>

<!-- Confirm apply dialog -->
<ConfirmDialog
  bind:open={confirmApplyOpen}
  title="Apply changes?"
  description="Apply {totalChanges} change{totalChanges > 1 ? 's' : ''} to {schema}.{table}?"
  confirmLabel="Apply"
  variant="default"
  loading={isApplying}
  onconfirm={applyChanges}
/>

<!-- Confirm discard dialog -->
<ConfirmDialog
  bind:open={confirmDiscardOpen}
  title="Discard changes?"
  description="You have unsaved changes. Discard all {totalChanges} pending changes?"
  confirmLabel="Discard"
  variant="destructive"
  onconfirm={() => {
    discardChanges();
    editMode = false;
  }}
/>

<!-- Export dialog -->
{#if canExport}
  <ExportDialog
    bind:open={exportOpen}
    {savedConnectionId}
    {databaseName}
    {schema}
    {table}
    whereClause={filterWhereClause}
  />
{/if}
