# Changelog

All notable changes to SakiDB will be documented in this file.

## Unreleased

### Added

- **Centralized context menus** — All 9 sidebar context menus (table, view, materialized view, function, sequence/index/foreign table, database, schema, connection, saved query) extracted into a single config-driven registry (`src/lib/context-menus/`). Menu items defined as data with capability-based `when` guards, rendered by a shared `ContextMenuRenderer` component. Eliminates hardcoded PostgreSQL SQL from ViewNode, MaterializedViewNode, FunctionNode, and ObjectInfoRow — all now route through the dialect system. Added `refreshMaterializedView()` to `SqlDialect` interface.
- **SQL dialect abstraction** — Engine-aware SQL generation on both frontend and backend, replacing all hardcoded PostgreSQL SQL.
  - Frontend `SqlDialect` interface (`src/lib/dialects/`) with `PostgresDialect` and `SqliteDialect` implementations. Factory function `getDialect()` with exhaustive engine switch. Covers DDL (add/alter/drop columns, indexes, foreign keys, triggers, partitions), DML (drop/truncate table, duplicate table, cell literals), and profiling queries.
  - Backend `SqlFormatter` trait (`sakidb-core`) with implementations for `PostgresDriver` (COPY format) and `SqliteDriver` (INSERT statements). Integrated into `DriverRegistry` and `export.rs` — SQL export now uses engine-specific DDL and data formatting instead of hardcoded PostgreSQL COPY.
  - All structure section components (Columns, Indexes, Relations, Triggers, Partitions), DataGrid, and profiling store migrated to use dialect system.
  - Deleted legacy `ddl.ts` and `profiling-sql.ts` utility files.
  - Query editor now uses dialect for CodeMirror language mode (`codemirrorDialect()`), SQL formatting (`formatterLanguage()`), and EXPLAIN wrapping (`explainAnalyzeQuery()`). Removed last hardcoded PostgreSQL references from `QueryTabView`.
- **SQLite driver** — Full SQLite support via `sakidb-sqlite` crate (rusqlite with bundled SQLite). Implements `Driver`, `SqlDriver`, `Introspector`, `Exporter`, and `Restorer` traits. Performance-tuned with WAL mode, 256 MB mmap, 64 MB cache, and native columnar query path.
- SQLite-specific commands — VACUUM and integrity check accessible from connection context menu.
- File picker for SQLite connections — browse button with `.db`/`.sqlite`/`.sqlite3`/`.db3` filter in both new connection and edit dialogs.
- **Multi-engine UX** — Engine selector in connection form, conditional form fields per engine (file-based engines hide host/port, Redis hides username, etc.).
- Engine label badge (`PG`, `SL`, `RD`, etc.) next to each connection name in the sidebar.
- `ConnectResult` bundles `runtime_id` + `EngineCapabilities` in a single IPC round-trip — no separate capabilities call needed.

### Performance

- **Paged columnar IPC** — new `execute_query_paged` command with columnar format for memory-efficient paginated queries.
- **DataGrid fast path** — optimized rendering for large result sets, avoiding unnecessary re-renders.
- **Store optimizations** — cached engine capabilities, bulk profiling SQL generation, debounced sidebar search, reactive tab index.
- **SQLite allocations** — reduced allocations in query execution and restore path (transaction batching).

### Changed

- **Comprehensive test coverage** — 260+ unit tests across all crates and frontend, feature-gated integration tests (postgres, sqlite), stress tests (1M rows, concurrency, cancellation), criterion benchmarks, and CI workflow with benchmark regression detection.
- **Testing conventions** — Documented in CLAUDE.md and CONTRIBUTING.md: `_test.rs` file convention, mock helpers, frontend store test patterns, and non-negotiable test requirements for all new code.
- **Test structure** — Migrated all 62 inline `#[cfg(test)]` blocks to dedicated `_test.rs` files across all crates. Tests now live side-by-side with implementation files but in separate modules, keeping source files clean.
- **Multi-driver architecture** — Split monolithic `DatabaseDriver` trait into composable traits (`Driver`, `SqlDriver`, `Introspector`, `Exporter`, `Restorer`, `KeyValueDriver`, `DocumentDriver`) for future database engine extensibility.
- Added `DriverRegistry` with connection routing — commands no longer import driver crates directly.
- All Tauri commands refactored to be engine-agnostic, routing through the registry.
- `sakidb-postgres` is now an optional dependency behind the `postgres` feature flag (enabled by default).
- Added `EngineType` enum, `EngineCapabilities` struct, and `available_engines` IPC command.
- Sidebar tree adapts to engine capabilities — database layer hidden for single-database engines, schema layer hidden for schema-less engines, category folders (Tables, Views, Functions, etc.) only render when the engine supports them.
- All context menus capability-gated — New Query (`sql`), Export (`export`), Restore (`restore`), Create/Drop/Rename Database (`multi_database`), View ERD (`introspection`).
- Query toolbar hides database selector when `!multi_database`, schema selector when `!schemas`, EXPLAIN buttons when `!explain`.
- Structure tab filters section tabs by capabilities (Indexes, Triggers, Partitions, Profiling).
- Export dialog hides SQL format option for non-SQL engines.

### Fixed

- **SQLite data tab not loading** — `tabIndex` stored plain objects instead of Svelte 5 `$state` proxies, so property mutations (`isLoading`, `queryResult`) never triggered UI re-renders. This affected all tab types but was most visible on SQLite where schema-related timing differences made it consistently reproducible.
- **SQLite schema-qualified queries failing** — All SQL generation (`SELECT`, `UPDATE`, `INSERT`, `DELETE`, `DROP`, `TRUNCATE`, export, profiling) used `"schema"."table"` format unconditionally. SQLite has no schemas (empty string), producing `""."table"` which SQLite misinterprets. Added `qualifiedTable()` helpers (frontend + Rust) that omit the schema prefix when empty.
- **Cancel not recognized for SQLite** — `isCancelError()` only matched PostgreSQL's cancel message. Now also matches SQLite's `"interrupted"` and core's `"Cancelled"`.
- **UTF-8 corruption in StreamingSqlSplitter** — byte-level splitting could break multi-byte characters during SQL restore; now splits on valid character boundaries.
- **SQL escaping hardened** — improved identifier and value escaping in generated SQL to prevent injection in edge cases.
- **tokio rt-multi-thread** — added missing feature flag for `block_in_place` support.
- ERD view rendering as blank canvas with 0% zoom — root cause was `flex-1` instead of `h-full` on ErdTabView, causing container height to collapse to 0.
- ERD minimap producing Infinity/NaN SVG attributes when zoom is 0.
- ERD fitToScreen not guarding against zero viewport dimensions.

## v1.1.0 (2026-03-06)

### Added

- Command palette (`Ctrl+K`) with fuzzy search, categories, recent commands, and keyboard shortcut hints.
- 40 keyboard shortcuts — `Ctrl+Enter` to run queries, `Ctrl+1–9` to switch tabs, `Ctrl+B` to toggle sidebar, and more.
- User-rebindable keybindings via Settings dialog (`Ctrl+,`) with conflict detection and reset-to-defaults.
- Command actions — save queries, switch databases/schemas, set query timeouts, create/drop/rename databases, change page sizes, trigger exports/imports, and view app info from the command palette.
- Table size in schema tree — each table shows disk size next to the row count estimate.
- Resizable sidebar (180–480px).
- Type-aware cell display — semantic colors and formatting for all PostgreSQL data types (numbers, booleans, temporals, UUIDs, network, JSON, geometric, search, XML).
- Specialized cell editors — boolean toggle popover, calendar picker for date/timestamp, floating JSON editor with validation and format/minify, UUID v7 generate button.
- SQL type casting — INSERT/UPDATE/DELETE now include proper casts (`::uuid`, `::inet`, `::interval`, etc.).

### Fixed

- Auto-updater now correctly detects and downloads new versions.
- Command palette arrow keys no longer skip items or stop working after typing.
- UUID columns now display correctly instead of `[binary: 16 bytes]`.


## v1.0.0 (2026-03-04)

Initial release.
