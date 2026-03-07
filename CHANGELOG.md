# Changelog

All notable changes to SakiDB will be documented in this file.

## Unreleased

### Added

- **Multi-engine UX** — Engine selector in connection form, conditional form fields per engine (file-based engines hide host/port, Redis hides username, etc.).
- Engine label badge (`PG`, `SL`, `RD`, etc.) next to each connection name in the sidebar.
- `ConnectResult` bundles `runtime_id` + `EngineCapabilities` in a single IPC round-trip — no separate capabilities call needed.

### Changed

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
