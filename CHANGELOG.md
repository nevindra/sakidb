# Changelog

All notable changes to SakiDB will be documented in this file.

## Unreleased

### CI/CD

- **Command palette** — Press `Ctrl+K` to open a fuzzy-searchable command palette with categories, recent commands, and keyboard shortcut hints.

- **40 keyboard shortcuts** — Navigate the app without a mouse: `Ctrl+Enter` to run queries, `Ctrl+1–9` to switch tabs, `Ctrl+B` to toggle the sidebar, and many more.

- **User-rebindable keybindings** — Customize any shortcut from the new Settings dialog (`Ctrl+,`), with conflict detection and reset-to-defaults.

- **Command actions** — Save queries, switch databases/schemas, set query timeouts, create/drop/rename databases, change page sizes, trigger exports/imports, and view app info — all from the command palette.

- **Table size in schema tree** — Each table now shows its disk size next to the row count estimate, so you can spot large tables at a glance.

- **Resizable sidebar** — Drag the right edge to adjust the sidebar width between 180–480px.

### Fixes

- **Auto-updater now works** — The in-app update check now correctly detects and downloads new versions.

- **Command palette keyboard navigation** — Arrow keys no longer skip items or stop working after typing in the search field.


## v1.0.0 (2026-03-04)

Initial release.
