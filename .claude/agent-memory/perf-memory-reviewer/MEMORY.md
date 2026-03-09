# Performance Review Memory

## Project Architecture
- Tauri v2 desktop app: Rust backend (crates/), Svelte 5 frontend (src/)
- Workspace crates: sakidb-core (traits/types), sakidb-postgres, sakidb-sqlite, sakidb-store
- IPC: MessagePack for query results, JSON for everything else
- Columnar format (ColumnStorage) for memory-efficient wire transfer
- CellValue uses Box<str>/Box<[u8]> to avoid String/Vec overhead -- good pattern
- CellValue is 24 bytes, Option<CellValue> also 24 bytes (niche optimization)
- ColumnStorage enum is 80 bytes (3 Vecs in largest variant)

## Key Patterns Found

### sakidb-core (reviewed 2026-03-08)
- `encode_into` Text/Bytes offset loop writes 4 bytes at a time -- should bulk-write like Number values
- `rows_to_columnar` column-at-a-time scan strides across flat cell array (cache-unfriendly) -- acceptable for fallback
- `rows_to_columnar` Text fallback uses `format!("{other:?}")` producing Debug output for mismatched types
- **BUG**: StreamingSqlSplitter `buf.push(ch as char)` mangles multi-byte UTF-8 (each byte treated as Latin-1)
- Null bitmaps use Vec<u8> (1 byte/row) -- 8x larger than bitpacked; tradeoff: simpler, matches wire format
- ColumnDef uses String (48 bytes/struct) where Box<str> (32 bytes) would suffice -- minor, not hot path

### sakidb-sqlite (reviewed 2026-03-08)
- `sqlite_value_to_cell` text path: `from_utf8_lossy().into_owned()` forces unnecessary copy on valid UTF-8
- `extract_column_defs`: calls `stmt.columns()` (allocates Vec) inside per-column loop
- `restore.rs flush_batch`: joins statements with ";\n" instead of executing individually in a transaction
- `restore.rs` line buffer: String allocated inside loop instead of reused with `.clear()`
- Row-reading loops duplicated across execute_query, execute_paged, execute_export
- `.map_err(|e| SakiError::QueryFailed(e.to_string()))` boilerplate -- candidate for From impl

### sakidb-postgres (reviewed 2026-03-08)
- ConnectionManager uses RwLock<HashMap> for pools -- should be DashMap (dashmap already a dep)
- RestoreProgress.clone() deep-copies error_messages Vec on every 100ms tick
- `Bytes::copy_from_slice` in COPY path doubles memory per line; use `std::mem::take`
- `vec![]` for empty params allocated on heap per query -- use `&[]` slice
- `.map_err(|e| SakiError::QueryFailed(...))` boilerplate repeated ~30x in introspect.rs

### DataGrid + data-view components (reviewed 2026-03-08)
- **HIGH**: Edit mode disables columnar fast path for ALL cells when any 1 cell is pending; fix: per-cell check
- **HIGH**: Map/Set/Array cloning on every edit for Svelte reactivity; fix: $state.raw + version counter
- **MED**: getCategoryCss called per text cell per render (toLowerCase + dict lookup); fix: cache per column
- **MED**: DataTab uses row-based QueryResultData, never columnar -- biggest memory win is paged columnar
- **MED**: getRowCells materializes full row for context menu/detail panel eagerly; fix: lazy/deferred
- **MED**: totalChanges + updateCount create redundant Sets; fix: compute once
- **MED**: deleteSelectedRows is O(N^2) due to per-iteration Set/Array cloning; fix: batch mutations
- **MED**: RowDetailPanel trackImageSrc accumulates ObjectURLs without bound; fix: revoke on row change
- **MED**: filterToSql manual SQL escaping misses backslash; correctness issue
- **LOW**: applyChanges maps columns per insert row; hoist above loop
- ObjectURL lifecycle fragmented across 3 components (CellDisplay, CellExpandPopover, RowDetailPanel)

### Good Practices Already in Place
- spawn_blocking for rusqlite calls (non-async library)
- Columnar path reads directly into typed columns, skipping CellValue intermediates
- Connection uses DashMap for concurrent access (sqlite)
- Export uses streaming batches with cancellation flag
- ColumnarResult::encode_into drops columns after writing to reduce peak memory
- encode() pre-allocates via estimate_size(); encode_into() intentionally skips reserve()
- One-shot SQL splitter returns borrowed &str slices (zero-alloc per statement)
- CancelTokenGuard RAII pattern (postgres)
- DataGrid: virtual scrolling with rAF-batched scroll, ResizeObserver cleanup, Uint32Array for sort indices
- DataGrid: getCellDisplay/getSortKey avoid CellValue construction on columnar path
- ColumnarResultData: lazy text decode with caching, class wrapper prevents Svelte deep-proxy
- DataGrid: contain:strict/content CSS on cells for layout isolation

### Svelte 5 Stores (reviewed 2026-03-08)
- **Unbounded caches**: schemaCompletionCache, completionBundleCache, completionColumnCache grow without limit
- **Linear tab lookups**: findTab() O(n) scan called 3x per query execution; needs Map<id, Tab> index
- **Profiling thundering herd**: loadProfilingData fires 3*N concurrent queries; needs concurrency limiter
- **Search O(n*m)**: hasDescendantMatch is O(matches) per tree node; pre-compute ancestor set
- **Good patterns**: Class wrappers prevent deep-proxying; tab eviction; runes=no subscription leaks; lazy text decode

### Cross-crate patterns
- Row-reading loop duplication in BOTH postgres and sqlite drivers
- Error mapping boilerplate in BOTH drivers -- candidate for From impl with newtype
- RestoreProgress cloning concern in postgres; check sqlite for same

## File Layout
- See CLAUDE.md for full architecture; test files are `*_test.rs` siblings
- Benchmarks in crates/*/benches/ using criterion
- Store files: src/lib/stores/*.svelte.ts (module-level $state, getter fns, async actions)
- Types: src/lib/types/index.ts (mirrors Rust), src/lib/types/query-result-data.ts (class wrappers)
- Data view: src/lib/components/data-view/ (DataGrid, CellDisplay, CellEditor, etc.)
- SQL utils: src/lib/sql-utils.ts (edit SQL gen, CellValue helpers)
