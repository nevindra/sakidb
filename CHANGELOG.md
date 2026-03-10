# Changelog

All notable changes to SakiDB will be documented in this file.

## Unreleased

### Added

- **Full object lifecycle menus** — Right-click any database object to create, drop, or manage it. Includes schema CRUD (create, rename, drop), "Create..." on every category folder (Tables, Views, Functions, Sequences, Indexes), and drop/reindex/reset actions on individual objects.
- **CASCADE option on drop** — Destructive actions now show an optional CASCADE checkbox (PostgreSQL) to also drop dependent objects.
- **Form-based object creation** — "Create..." actions now open dedicated form dialogs instead of raw SQL templates. Create tables, views, materialized views, functions, sequences, and indexes through an intuitive UI with live DDL preview.
- **Edit database objects** — Right-click views, materialized views, functions, or sequences to edit them in place. Each opens a dialog pre-filled with the object name for quick modifications.
- **Query result export** — Export query results to CSV or JSON files directly from the query editor, without going through the Data tab.
- **Copy results to clipboard** — One-click copy of query results as tab-separated values, ready to paste into Excel or Google Sheets.
- **Result comparison** — Compare two results from a multi-statement query side by side. Highlights added, removed, and changed cells. Match rows by position or by a key column of your choice.
- **Column count in status bar** — The query result bar now shows the number of columns alongside the row count.
- **SQLite support** — Connect to local `.db`, `.sqlite`, and `.sqlite3` files with full browsing, editing, export, and restore. Includes VACUUM and integrity check commands.
- **Multi-engine connections** — Engine selector in the connection form with adaptive fields per engine type. Engine badge (`PG`, `SL`, etc.) shown next to each connection in the sidebar.
- **Improved structure dialogs** — Searchable type picker for column data types, multi-select dropdowns for index and foreign key columns, and polished inputs across all structure panels (Columns, Indexes, Relations, Triggers, Partitions).
  - Add Column dialog now supports primary key, unique, nullable, array, precision, check constraint, and comment options.
- **Smarter context menus** — Right-click menus across all sidebar items now adapt to each engine's capabilities. Unavailable actions are automatically hidden.
- **File picker for SQLite** — Browse button with database file filter in connection dialogs.

### Performance

- **Instant sidebar expansion** — Opening large lists (1000+ functions, indexes, etc.) is now near-instant. Virtualized rendering only draws visible rows; dialogs are lazy-mounted on demand.
- Paginated queries for large result sets with lower memory usage.
- Faster DataGrid rendering, avoiding unnecessary re-renders.
- Optimized SQLite query execution and restore performance.

### Changed

- UI adapts to engine capabilities — sidebar tree, query toolbar, structure tabs, and export dialog show only what the connected engine supports.
- 260+ unit tests added across backend and frontend.

### Fixed

- UTF-8 corruption when restoring SQL files with multi-byte characters.
- SQL escaping hardened to prevent edge-case injection.
- ERD view rendering as blank canvas at 0% zoom.
- Empty category folders (Tables, Views, Functions, etc.) disappearing from the sidebar after loading instead of showing a zero count.
- Opening the same table name from different databases reusing an existing tab instead of opening a new one.
- ERD minimap and fit-to-screen errors with zero dimensions.

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
