# Contributing to SakiDB

This guide applies to all contributors — human and LLM alike. Read it before writing any code.

For detailed codebase reference (architecture, commands, file paths), see `CLAUDE.md`.

---

## Identity & Philosophy

SakiDB is a database client that feels like a professional instrument, not a web app. Fast to open, fast to use, fast to get out of. The UI never draws attention to itself — the data is always the protagonist.

**Visual DNA:** Linear-style neutral. Layered neutral-gray surfaces (`#1a1a1e` background, `#1f1f23` card, `#222226` popover), cool desaturated grays, tight radii, SakiDB red accent (`#d93b3b`). Dark-first. Ghost-style inputs (borderless by default, border on focus). The aesthetic says "I was built by someone who cares" without saying "look at me."

**Core design beliefs:**

- **Data is the protagonist.** Every pixel of chrome exists to serve the data grid and query editor. When in doubt, give more space to content, less to controls.
- **Speed is felt, not measured.** Users don't read benchmarks. They feel lag. The results table and scroll must never drop frames. But a dialog can have a tasteful 150ms fade-in — that actually *feels* faster because it provides continuity.
- **Progressive density.** Show less by default, reveal on demand. A workspace with 50 tables should feel organized, not overwhelming. Information appears when you need it (hover, expand, focus) and stays out of the way when you don't.
- **Respect the platform.** This is a desktop app, not a website. Native keyboard shortcuts, proper window management, no web-isms leaking through.

---

## Non-Negotiable Rules

These rules are absolute. PRs that violate them will be rejected.

### Performance

- The results table, virtual scroll, query editor typing, and tree node expansion MUST maintain 60fps. These are hot paths — no exceptions.
- Query results MUST use MessagePack serialization (`rmp_serde` in Rust, `@msgpack/msgpack` in TypeScript). JSON is fine for metadata (connections, schemas). This binary/JSON IPC split is a permanent architectural decision — do not revisit it.
- DO NOT add synchronous blocking operations to hot paths. If the user is *waiting* on it, make it instant. If the user *triggered* it, make it smooth (up to 200ms for tasteful motion).

### UI Components

- You MUST use shadcn-svelte for all UI primitives: buttons, inputs, selects, dialogs, dropdowns, tooltips, popovers, context menus, checkboxes, switches, sheets, scroll areas.
- DO NOT create custom components for anything shadcn already provides. Check `src/lib/components/ui/` first.
- Custom-built components are allowed ONLY for domain-specific things: query editor (CodeMirror 6), results grid (virtual-scrolled), schema tree, ERD canvas.
- **The test:** If a component exists in a generic design system, use shadcn. If it only makes sense inside a database client, build it.

### Testing

- Every new Rust module MUST have a corresponding `_test.rs` file with tests covering its public API. No exceptions.
- Every new Tauri command MUST have tests in the corresponding `commands/*_test.rs` file.
- Every new frontend store function MUST have tests in `src/lib/stores/__tests__/`.
- DO NOT merge code that reduces test coverage. If you add a function, add a test. If you change behavior, update the test.
- Integration and stress tests are feature-gated — they don't run in CI by default but MUST pass before release.
- `cargo test` and `pnpm test` MUST pass before any PR is merged. CI enforces this.

---

## Frontend Guidelines

### Component Reusability

Components should be database-agnostic. SakiDB will support multiple database engines — a component that works for Postgres today should work for MySQL tomorrow without modification.

- Keep database-specific logic in store modules, not in components. Components receive data through props — they don't know or care which driver produced it.
- If you're building a component that displays columns, rows, indexes, or constraints, design it against the types in `src/lib/types/index.ts` (which mirror `sakidb-core` Rust types), not against Postgres-specific structures.
- When a component needs database-specific behavior (e.g., Postgres-only features), use conditional rendering based on data, not hardcoded assumptions.

### Context Menus

All sidebar context menus are centralized in `src/lib/context-menus/`. DO NOT add inline `ContextMenu.Content` with hardcoded items in components.

- **Menu definitions** live in `menu-items.ts` — each node type has a function returning `MenuEntry[]` with capability-based `when` guards.
- **Rendering** uses `ContextMenuRenderer.svelte` — pass items, a `MenuContext`, and an `onaction` callback.
- **Action handlers** live in the node component as a `switch` on the menu item `id`. Dialogs stay in node components where their state lives.
- **SQL generation** must go through the dialect system (`getDialect()`) — never hardcode `"${schema}"."${table}"` or engine-specific SQL in components.
- To add a menu item: add an entry to the relevant function in `menu-items.ts`, then handle the new `id` in the component's action handler.
- To add menus for a new node type: add a new function to `menu-items.ts`, use `<ContextMenuRenderer>` in the component.

### Svelte Conventions

- Svelte 5 runes only: `$state()`, `$derived()`, `$effect()`, `$props()`. DO NOT use legacy Svelte stores or reactive declarations (`$:`, `let x; $: x = ...`).
- Global state lives in domain `.svelte.ts` files under `src/lib/stores/`, composed via `getAppState()`. Do not create new state patterns.

### Styling

- Tailwind v4 utility classes throughout. No component-scoped CSS unless you have a specific animation need.
- Use the design tokens defined in `@theme` in `app.css` (neutral gray palette with SakiDB red accent). Do not introduce new colors outside the theme.

### Consistency

A button is a button everywhere. An input is an input everywhere. If your new feature needs a UI element that looks or behaves differently from the same element elsewhere, that's a bug in your design, not a special case.

---

## Backend Guidelines

### The Trait Is the Extension Point

Adding a new database engine means creating a new crate that implements composable traits from `sakidb-core`: `Driver` (required) plus optional capability traits (`SqlDriver`, `Introspector`, `Exporter`, `Restorer`, `SqlFormatter`). For SQL engines, also add a frontend `SqlDialect` in `src/lib/dialects/` — this handles engine-specific SQL generation (DDL, DML, cell literals, profiling queries). The UI adapts automatically via `EngineCapabilities`. See **[docs/adding-a-driver.md](docs/adding-a-driver.md)** for the full step-by-step guide.

- All shared types live in `sakidb-core` (`CellValue`, `ColumnarResult`, `QueryResult`, `ErdData`, `SakiError`). New database crates depend on `sakidb-core` and nothing else from the workspace.
- DO NOT add database-specific types to `sakidb-core`. If only Postgres needs it, it lives in `sakidb-postgres`.

### Crate Boundaries Are Compile-Time Guarantees

The multi-crate workspace isn't organizational preference — it enforces that concerns don't leak. `sakidb-core` can't accidentally depend on Postgres. A driver crate can't touch credential storage. If it compiles, the boundaries hold.

- `sakidb-core` — traits, shared types, errors
- `sakidb-postgres` — Postgres driver implementation
- `sakidb-store` — credential storage, saved queries, query history
- `src-tauri` — Tauri commands (glue layer only)

### Keep the Tauri Command Layer Thin

Tauri commands are glue: validate input, call the driver or store, serialize the response. Business logic lives in the crates. If a command function is getting long, logic is leaking into the wrong layer.

### Concurrency

The store mutex is a gate — acquire, read/write, drop before any `await`. The driver manages its own concurrency through `Arc<RwLock>` for connection pools. If you can't explain the locking strategy in one sentence, simplify it.

### Backend Testing Conventions

- Unit tests go in `_test.rs` sibling files (e.g., `executor.rs` → `executor_test.rs`), declared with `#[cfg(test)] mod executor_test;` in `lib.rs`. DO NOT put tests inline in source files.
- Test only `pub` and `pub(crate)` APIs. If something is private, test it through its public interface.
- Tauri command tests use `mock_helpers::create_test_state()` which provides an `AppState` with an empty `DriverRegistry` and a temp-file `Store`. Test store operations directly — do not attempt to wrap in Tauri's `State<'_>`.
- Integration tests requiring real databases go in `crates/*/tests/integration.rs` behind `#![cfg(feature = "integration")]`. Use `TEST_DATABASE_URL` env var for Postgres.
- Stress tests go in `crates/*/tests/stress.rs` behind `#![cfg(feature = "stress")]`.
- Benchmarks use criterion in `crates/*/benches/`. Postgres benchmarks skip gracefully when `TEST_DATABASE_URL` is not set.

---

## Frontend Testing Guidelines

- Store tests live in `src/lib/stores/__tests__/*.test.ts`.
- Tauri IPC calls are mocked globally in `src/lib/stores/__tests__/setup.ts` via `vi.mock('@tauri-apps/api/core')`. Add new mock responses there when adding new commands.
- Use `makeConnectResult()` and `makeCapabilities()` factories from `setup.ts` for connection test data — never hardcode mock shapes inline.
- Each test must get fresh store state: use `vi.resetModules()` + dynamic `import()` to avoid Svelte 5 rune state leaking between tests.
- Test behavior, not implementation. Assert on what the user sees (state values, IPC calls made), not on internal store structure.

---

## Commit Conventions

Use [conventional commits](https://www.conventionalcommits.org/) with these prefixes:

- `feat:` — new feature
- `fix:` — bug fix
- `perf:` — performance improvement
- `refactor:` — code change that neither fixes a bug nor adds a feature
- `chore:` — build, CI, dependency updates
- `docs:` — documentation only

Keep the subject line under 70 characters. Focus on *why*, not *what* — the diff already shows what changed.
