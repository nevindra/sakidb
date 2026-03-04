<p align="center">
  <img src="src-tauri/icons/icon.png" width="80" />
</p>

<h1 align="center">Saki</h1>

<p align="center">
  A lightning-fast PostgreSQL client that stays out of your way.
</p>

<p align="center">
  <a href="https://github.com/nevindra/sakidb/releases"><img src="https://img.shields.io/github/v/release/nevindra/sakidb?style=flat-square&label=download" alt="Latest Release" /></a>
  <a href="https://github.com/nevindra/sakidb/blob/main/LICENSE"><img src="https://img.shields.io/github/license/nevindra/sakidb?style=flat-square" alt="License" /></a>
  <a href="https://github.com/nevindra/sakidb/stargazers"><img src="https://img.shields.io/github/stars/nevindra/sakidb?style=flat-square" alt="Stars" /></a>
</p>

<p align="center">
  <a href="#install">Install</a>&nbsp;&nbsp;&middot;&nbsp;&nbsp;<a href="#features">Features</a>&nbsp;&nbsp;&middot;&nbsp;&nbsp;<a href="#contributing">Contributing</a>&nbsp;&nbsp;&middot;&nbsp;&nbsp;<a href="#built-with">Built With</a>
</p>

<br />

<p align="center">
  <img src="docs/screenshot/home.png" width="800" alt="Saki — schema explorer and connection tree" />
</p>

<br />

## Why Saki

**Native speed.** Instant startup, binary IPC for query results. No Electron. No web browser in disguise. Just a fast app.

**Data is the protagonist.** The interface recedes so your results command attention. Virtual-scrolled grid, resizable columns, inline editing — built for real datasets, not demo tables.

**Your secrets stay safe.** Passwords never leave the backend. Not in logs, not over IPC, not ever.

<br />

## Features

### Query Editor

Full SQL editor with PostgreSQL syntax highlighting, schema-aware autocomplete, and multi-statement execution. Run everything, or just the statement at your cursor. Built-in EXPLAIN ANALYZE visualizer with table, tree, and raw views.

<p align="center">
  <em>Screenshot coming soon</em>
</p>

### Data Grid

Virtual-scrolled results table that handles thousands of rows without breaking a sweat. Click a cell to expand, double-click a row for full detail. Sort columns, filter with SQL conditions, edit inline — with undo support and batch commits.

<p align="center">
  <em>Screenshot coming soon</em>
</p>

### Schema Explorer

Browse your entire database structure in a hierarchical tree — databases, schemas, tables, views, materialized views, functions, sequences, foreign tables. Fuzzy search across everything. Right-click for quick actions.

<p align="center">
  <em>Screenshot coming soon</em>
</p>

### ERD Viewer

Auto-layout entity relationship diagrams. Pan, zoom, drag nodes, hover to highlight foreign key connections. Minimap for orientation. Double-click a table to jump straight to its data.

<p align="center">
  <em>Screenshot coming soon</em>
</p>

<br />

### And also...

- **Command palette** — fuzzy-search any action, keyboard-first workflow
- **Saved queries & history** — auto-saved execution history, promote queries to your library
- **Export** — CSV or full SQL dump (DDL + data), with streaming for large tables
- **SQL restore** — import `.sql` files with progress tracking and error handling
- **Database management** — create, drop, and rename databases without leaving the app
- **Query cancellation** — cancel long-running queries with a PostgreSQL cancel signal
- **Multi-result tabs** — run multiple statements, navigate results with Alt+Arrow
- **Table structure** — columns, indexes, foreign keys, triggers, partitions, DDL preview
- **Connection management** — save connections securely, test before connecting

<br />

## Install

### Download

Grab the latest release for your platform:

<p align="center">
  <a href="https://github.com/nevindra/sakidb/releases/latest"><strong>Download Saki &rarr;</strong></a>
</p>

| Platform | File |
|----------|------|
| **macOS** (Apple Silicon) | `.dmg` |
| **macOS** (Intel) | `.dmg` |
| **Linux** (Debian/Ubuntu) | `.deb` |
| **Linux** (AppImage) | `.AppImage` |
| **Windows** | `.msi` |

### Build from source

Requires [Rust](https://rustup.rs/), [Node.js](https://nodejs.org/), and [pnpm](https://pnpm.io/).

```bash
git clone https://github.com/nevindra/sakidb.git
cd sakidb
pnpm install
pnpm tauri build
```

The built application will be in `src-tauri/target/release/bundle/`.

<br />

## Built With

Tauri v2 &nbsp;+&nbsp; Svelte 5 &nbsp;+&nbsp; Rust

<br />

## Contributing

Contributions are welcome! Whether it's bug reports, feature requests, or pull requests — all input helps make Saki better.

### Getting started

```bash
git clone https://github.com/nevindra/sakidb.git
cd sakidb
pnpm install
pnpm tauri dev
```

### Project structure

```
crates/sakidb-core/       — Shared traits, types, errors
crates/sakidb-postgres/   — PostgreSQL driver
crates/sakidb-store/      — Encrypted credential & query storage
src-tauri/                — Tauri app, IPC commands
src/                      — Svelte 5 frontend
```

### Running tests

```bash
cargo test          # Rust tests
pnpm check          # TypeScript/Svelte type checking
```

### Before submitting a PR

- Run `cargo clippy` and fix any warnings
- Run `pnpm check` to ensure type safety
- Keep commits focused and descriptive

<br />

Have an idea? [Open an issue](https://github.com/nevindra/sakidb/issues/new) to start a discussion.

<br />

## License

[MIT](LICENSE)
