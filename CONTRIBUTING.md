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

---

## Frontend Guidelines

### Component Reusability

Components should be database-agnostic. SakiDB will support multiple database engines — a component that works for Postgres today should work for MySQL tomorrow without modification.

- Keep database-specific logic in store modules, not in components. Components receive data through props — they don't know or care which driver produced it.
- If you're building a component that displays columns, rows, indexes, or constraints, design it against the types in `src/lib/types/index.ts` (which mirror `sakidb-core` Rust types), not against Postgres-specific structures.
- When a component needs database-specific behavior (e.g., Postgres-only features), use conditional rendering based on data, not hardcoded assumptions.

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

Adding a new database engine means creating a new crate that implements `DatabaseDriver` from `sakidb-core`. Zero changes to core, zero changes to frontend. If a feature can't be expressed through the trait, rethink the feature before breaking the abstraction.

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

---

## Commit Conventions

Use [conventional commits](https://www.conventionalcommits.org/). The changelog is auto-generated by git-cliff from these prefixes:

- `feat:` — new feature
- `fix:` — bug fix
- `perf:` — performance improvement
- `refactor:` — code change that neither fixes a bug nor adds a feature
- `chore:` — build, CI, dependency updates
- `docs:` — documentation only

Keep the subject line under 70 characters. Focus on *why*, not *what* — the diff already shows what changed.
