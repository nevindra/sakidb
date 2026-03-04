# SakiDB Design Principles

## Identity

SakiDB is a database client that feels like a professional instrument, not a web app. Fast to open, fast to use, fast to get out of. The UI never draws attention to itself; the data is always the protagonist.

**Visual DNA:** Linear-style neutral. Near-black layered surfaces, desaturated grays, tight radii, indigo accent. Dark-first. The aesthetic says "I was built by someone who cares" without saying "look at me."

## Core Principles

**1. Data is the protagonist.**
Every pixel of chrome — sidebar, toolbar, tab bar — exists to serve the data grid and query editor. When in doubt, give more space to content, less to controls. UI elements should recede; results should command attention.

**2. Speed is felt, not measured.**
Users don't read benchmark numbers. They feel lag. Prioritize *perceived* performance: the results table and scroll must never drop frames. But a connection dialog can have a tasteful 150ms fade-in — that actually *feels* faster because it provides continuity. Animations serve feedback, never decoration.

**3. Progressive density.**
Show less by default, reveal on demand. Empty states should feel calm, not barren. A connected workspace with 50 tables should feel organized, not overwhelming. Information appears when you need it (hover, expand, focus) and stays out of the way when you don't.

**4. Consistent surfaces.**
Everything that looks the same should behave the same. A button is a button everywhere. An input is an input everywhere. shadcn-svelte is the foundation — one component library means one visual language. Custom components only for domain-specific things (query editor, results grid, schema tree).

**5. Respect the platform.**
SakiDB is a desktop app, not a website. Native-feeling keyboard shortcuts, proper window management, no "web-isms" like page scrolling or URL bars leaking through. It should feel like it belongs on the user's OS.

## Performance Philosophy

**The 60fps contract.** The results table, virtual scroll, and any interaction the user repeats rapidly (typing in the editor, expanding tree nodes, resizing panels) must maintain 60fps. This is non-negotiable. These are the "hot paths."

**The polish budget.** Everything else — dialogs opening, toasts appearing, sidebar transitions, connection state changes — gets a budget of up to 200ms for tasteful motion. These moments benefit from polish because they provide spatial continuity and feedback. A dialog that snaps open feels broken; one that eases in 150ms feels solid.

**The rule:** If the user is *waiting* on it, make it instant. If the user *triggered* it, make it smooth.

**Binary IPC stays.** MessagePack for query results is a core architectural decision, not an optimization to revisit. JSON is fine for metadata. This split is permanent.

## Component Strategy

**shadcn-svelte is the foundation.** All primitive UI components — buttons, inputs, selects, dialogs, dropdowns, tooltips, popovers, context menus — come from shadcn-svelte. Themed to match SakiDB's identity. When you need a new interactive primitive, check shadcn first.

**Custom-built only for domain components:**
- Query editor (CodeMirror 6)
- Results table (virtual-scrolled data grid — performance-critical)
- Schema tree (domain-specific interaction patterns)

**The test:** If a component exists in a generic design system, use shadcn. If it only makes sense inside a database client, build it.

## Backend Engineering Principles

**1. Performance is architectural, not afterthought.**
The fast path — query execution through IPC to the frontend — is designed for speed at every layer: async streaming from Postgres, MessagePack serialization, binary IPC. Architectural decisions that protect this path (like the binary/JSON IPC split) are permanent. New features that touch the query path must preserve these guarantees.

**2. Crate boundaries enforce maintainability.**
The multi-crate workspace isn't organizational preference — it's a compile-time guarantee that concerns don't leak. `sakidb-core` can't accidentally depend on Postgres. A driver crate can't accidentally touch credential storage. If the code compiles, the boundaries hold. Refactoring one crate never cascades into unrelated crates.

**3. The trait is the extension point.**
`DatabaseDriver` defines what a database backend can do. Adding MySQL or SQLite means a new crate — zero changes to core, zero changes to frontend. If a feature can't be expressed through the trait, rethink the feature before breaking the abstraction.

**4. Keep the layers thin.**
Tauri commands are glue: validate input, call the driver or store, serialize the response. Business logic lives in the crates. If a command is getting long, logic is leaking into the wrong layer. Thin layers are easy to read, easy to test, easy to replace.

**5. Concurrency is explicit, not clever.**
The store mutex is a gate (locked/unlocked state), not a long-held lock — acquire, read/write, drop before any `await`. The driver manages its own concurrency through `Arc<RwLock>` for connection pools. No hidden shared state. If you can't explain the locking strategy in one sentence, simplify it.

**6. Secrets stay in Rust.**
Passwords are decrypted only in the backend, only when needed, and never cross IPC. The frontend sends plaintext only during save — the store encrypts immediately. This is enforced by `#[serde(skip)]` on encrypted fields.
