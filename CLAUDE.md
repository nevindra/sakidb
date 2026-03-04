# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is SakiDB

SakiDB is a lightweight, fast, low-memory desktop PostgreSQL client. Built with Tauri v2 (Rust backend) + Svelte 5 + TypeScript frontend. The architecture is designed for future database engine extensibility via a trait-based driver system.

## Design Principles

Read [docs/DESIGN_PRINCIPLES.md](docs/DESIGN_PRINCIPLES.md) before making any UI or architectural decisions. Key points:

- **Data is the protagonist** — UI recedes, results command attention
- **Speed is felt, not measured** — 60fps on hot paths (results table, scroll, editor), tasteful motion elsewhere
- **Progressive density** — show less by default, reveal on demand
- **shadcn-svelte is the component foundation** — custom only for domain components (query editor, results grid, schema tree)
- **Performance is architectural** — MessagePack IPC for queries, crate boundaries enforce separation, thin Tauri command layers
- **Secrets stay in Rust** — passwords never cross IPC

## Commands

### Development
```bash
pnpm tauri dev          # Run the full app (starts Vite dev server + Rust backend)
pnpm dev                # Frontend only (Vite dev server on port 1420)
pnpm build              # Frontend build only (SvelteKit static adapter -> build/)
pnpm check              # TypeScript/Svelte type checking
```

### Rust
```bash
cargo build                              # Build all workspace crates
cargo build -p sakidb-core               # Build a single crate
cargo test                               # Run all tests
cargo test -p sakidb-store               # Test a single crate
cargo test -p sakidb-core test_name      # Run a specific test
cargo clippy                             # Lint
```

### Tauri
```bash
pnpm tauri build        # Production build (runs pnpm build, then compiles Rust release + bundles)
```

## Architecture

### Rust Workspace (`Cargo.toml` workspace members: `src-tauri`, `crates/*`)

```
crates/sakidb-core/       — Shared traits, types, errors. Everything depends on this.
crates/sakidb-postgres/   — PostgreSQL driver (tokio-postgres + deadpool-postgres). Implements DatabaseDriver trait.
crates/sakidb-store/      — Encrypted credential storage (rusqlite + AES-256-GCM). Also stores saved queries & query history.
src-tauri/                — Tauri app. Wires drivers + store into IPC commands.
```

**Extension point:** Adding a new database backend means creating a new crate implementing `DatabaseDriver` from `sakidb-core`. No changes needed to core or frontend.

### Key Rust types

- `DatabaseDriver` trait (`crates/sakidb-core/src/driver.rs`) — 22 async methods covering:
  - Connection: `connect`, `disconnect`, `test_connection`
  - Query execution: `execute`, `execute_multi`, `execute_paged`, `execute_batch`, `cancel_query`
  - Introspection: `list_databases`, `list_schemas`, `list_tables`, `list_columns`, `list_views`, `list_materialized_views`, `list_functions`, `list_sequences`, `list_indexes`, `list_foreign_tables`, `list_triggers`, `list_foreign_keys`, `list_check_constraints`, `list_unique_constraints`, `get_partition_info`, `get_create_table_sql`, `get_erd_data`
- `CellValue` enum (`crates/sakidb-core/src/types.rs`) — Null, Bool, Int, Float, Text, Bytes, Json, Timestamp. Used instead of serde_json::Value for performance.
- `ColumnarResult` / `ColumnarResultData` (`crates/sakidb-core/src/types.rs`) — memory-efficient columnar storage format with typed columns (Number, Bool, Text, Bytes) and null bitmaps.
- `MultiQueryResult` (`crates/sakidb-core/src/types.rs`) — wraps multiple `QueryResult` objects for multi-statement execution.
- `ErdData` (`crates/sakidb-core/src/types.rs`) — entity-relationship diagram data for schema visualization.
- `SakiError` enum (`crates/sakidb-core/src/error.rs`) — variants: ConnectionFailed, QueryFailed, AuthFailed, Timeout, Cancelled, StorageError, EncryptionError, NotConnected, ConnectionNotFound. Derives both `thiserror::Error` and `Serialize` so errors cross the IPC boundary.
- `AppState` (`src-tauri/src/state.rs`) — `Arc<PostgresDriver>` + `Arc<Mutex<Store>>` + `Arc<AtomicBool>` (restore cancellation) + `Arc<DashMap<ConnectionId, Arc<AtomicBool>>>` (per-connection export cancellation).

### Tauri Commands (`src-tauri/src/commands/`)

~44 IPC commands across six modules:
- `connection.rs` — CRUD saved connections, connect/disconnect/test, database management (create/drop/rename)
- `query.rs` — execute_query, execute_query_multi, execute_query_multi_columnar, execute_query_paged, execute_batch, cancel_query
- `explorer.rs` — list_databases, list_schemas, list_tables, list_columns, list_views, list_materialized_views, list_functions, list_sequences, list_indexes, list_foreign_tables, list_triggers, list_foreign_keys, list_check_constraints, list_unique_constraints, get_partition_info, get_create_table_sql, get_erd_data
- `export.rs` — export_table_csv, export_table_sql, cancel_export
- `import.rs` — restore_from_sql, cancel_restore
- `queries.rs` — save_query, list_saved_queries, update_saved_query, delete_saved_query, add_query_history, list_query_history, clear_query_history, save_from_history

### Svelte Frontend (`src/`)

```
src/lib/stores/             — Modular state (Svelte 5 runes), barrel-exported via index.ts → getAppState()
  ├── index.ts              — Barrel export composing getAppState() from domain modules
  ├── connections.svelte.ts — Connection CRUD, connect/disconnect, schema tree loading
  ├── query-tab.svelte.ts   — Query execution, saved queries, query history
  ├── data-tab.svelte.ts    — Data table browsing with filtering & pagination
  ├── structure-tab.svelte.ts — Table structure, DDL, ERD, profiling
  ├── tabs.svelte.ts        — Tab lifecycle (open/close/switch) for all tab types
  ├── search.svelte.ts      — Schema tree search/filter
  ├── shared.svelte.ts      — Global error state
  └── exports.svelte.ts     — CSV/SQL export, SQL restore
src/lib/types/index.ts      — TypeScript mirrors of all Rust types
src/lib/components/         — UI components (organized by domain)
src/routes/+page.svelte     — Root page: main layout
```

**Component domains** (`src/lib/components/`):
```
shell/          — TitleBar, TabBar, CommandPalette (Ctrl+K), Toast
sidebar/        — Sidebar, ConnectionManager, ConnectionEditDialog, ConnectionTree, QueryList
  └── tree/     — DatabaseNode, TableNode, RestoreDialog, etc.
query-editor/   — QueryTabView, QueryToolbar, QueryResultBar, ResultTabBar, ExplainViewer (tree + table)
data-view/      — DataTabView, DataGrid, CellDisplay, CellEditor, CellExpandPopover, GridFilterBar, GridContextMenu, GridBottomBar, RowDetailPanel
structure/      — StructureTabView, ColumnsSection, IndexesSection, RelationsSection, TriggersSection, PartitionsSection, ProfilingSection, DdlPreview, ExportDialog, BarChart
erd/            — ErdTabView, ErdTableNode (entity-relationship diagram visualization)
ui/             — shadcn-svelte primitives (button, input, dialog, dropdown-menu, tooltip, sheet, select, checkbox, switch, etc.)
```

### Tab System

Four tab types, each with its own view component and store logic:
- **QueryTab** — CodeMirror 6 editor, multi-statement execution, result tabs, EXPLAIN viewer
- **DataTab** — virtual-scrolled data grid with inline filtering, pagination, cell editing, row detail panel
- **StructureTab** — table metadata sections (columns, indexes, relations, triggers, partitions, profiling, DDL)
- **ErdTab** — entity-relationship diagram for a schema

### IPC Serialization — Two Paths

- **Query results** use MessagePack (`rmp_serde` Rust → `@msgpack/msgpack` TS) for performance. Commands return `Vec<u8>`, frontend decodes in store modules. Columnar format (`execute_query_multi_columnar`) provides additional memory efficiency.
- **Everything else** (connections, schemas, etc.) uses Tauri's default JSON serialization.
- **Progress events** for long-running operations (export, restore) use Tauri event system.

### Two-Level Connection ID System

- `SavedConnection.id` — persisted UUID in SQLite (used for UI state)
- Runtime `ConnectionId` — UUID from the in-memory deadpool-postgres registry (used for all IPC query routing)
- `getRuntimeId()` helper in stores bridges saved → runtime IDs

### Credential Storage

Stored in `~/.local/share/sakidb/config.db` (SQLite). Only the password field is AES-256-GCM encrypted per-row. Master password → PBKDF2 (100K iterations) → 256-bit key, held in memory for session duration. Same SQLite DB also stores saved queries and query history.

## CI/CD

### Release Flow (`git tag v* → push`)
1. **`changelog.yml`** — git-cliff regenerates `CHANGELOG.md`, commits to `main`. Unreleased commits move under the new version heading.
2. **`release.yml`** — Builds 4 targets (Linux x86_64, macOS aarch64, macOS x86_64, Windows x86_64) via `tauri-action`. Creates a **draft** GitHub Release with artifacts attached. Release notes are written manually.

### Changelog Flow (`cliff.toml`)
- Uses conventional commits (`feat:`, `fix:`, `perf:`, etc.) to auto-generate changelog via git-cliff
- PR merged to `main` → changelog updated with new commits under `## Unreleased`
- Tag pushed → unreleased commits promoted to `## vX.Y.Z (date)`

## Conventions

### Rust
- `?` operator for error propagation with `SakiError`
- Unit tests live in the same file as the code they test (not separate test files)
- `sakidb-store` uses `Store::open_in_memory()` for tests
- Commands acquire `state.store.lock().await` and operate on the store directly
- `sakidb-postgres` modules: `connection.rs` (pool management), `executor.rs` (query execution), `introspect.rs` (schema introspection), `restore.rs` (SQL restore)

### TypeScript / Svelte
- Svelte 5 runes only: `$state()`, `$derived()`, `$effect()`, `$props()`. No legacy stores.
- Global state uses module-level `$state()` in domain `.svelte.ts` files, composed via `getAppState()` in `stores/index.ts`
- TypeScript interfaces mirror Rust struct field names exactly (snake_case: `rows_affected`, `ssl_mode`, etc.)
- `$lib` alias resolves to `src/lib/`

### CSS
- Tailwind v4 with `@theme` design tokens in `app.css` (Catppuccin Mocha dark palette)
- Utility-first throughout, no component CSS except Toast animation

### Naming
- camelCase in TypeScript/Svelte, snake_case in Rust
- Tauri command names are snake_case on both sides
