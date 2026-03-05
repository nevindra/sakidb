# Changelog

All notable changes to SakiDB will be documented in this file.

## Unreleased

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
