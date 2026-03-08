# Adding a New Database Driver

This guide walks through adding a new database engine to SakiDB. The architecture is designed so that **zero changes to core types or frontend code** are needed — you implement traits, register the driver, and the UI adapts automatically via `EngineCapabilities`.

**Reference implementation:** `sakidb-sqlite` is the simplest complete driver. Read it alongside this guide.

---

## Overview

```
1. Add EngineType variant          → crates/sakidb-core/src/types.rs
2. Create driver crate             → crates/sakidb-<engine>/
3. Implement traits                → Driver (required) + optional traits
4. Set EngineCapabilities          → tells frontend what to show/hide
5. Feature-flag in src-tauri       → src-tauri/Cargo.toml
6. Register in DriverRegistry      → src-tauri/src/lib.rs
7. Add engine-specific commands    → src-tauri/src/commands/ (if needed)
8. Add tests                       → unit, integration, benchmarks
```

---

## Step 1: Add EngineType variant

In `crates/sakidb-core/src/types.rs`, add your engine to the `EngineType` enum:

```rust
pub enum EngineType {
    Postgres,
    Sqlite,
    Redis,
    MongoDB,
    DuckDB,
    ClickHouse,
    YourEngine,  // ← add here
}
```

Update the `Display` and `FromStr` impls in the same file to handle the new variant. The string representation must be lowercase (e.g., `"yourengine"`).

---

## Step 2: Create the driver crate

```bash
mkdir -p crates/sakidb-<engine>/src
```

Create `crates/sakidb-<engine>/Cargo.toml`:

```toml
[package]
name = "sakidb-<engine>"
version = "1.0.0"
edition = "2021"

[dependencies]
sakidb-core = { path = "../sakidb-core" }
async-trait = "0.1"
tokio = { version = "1", features = ["rt"] }
uuid = { version = "1", features = ["v4"] }
dashmap = "6"
tracing = "0.1"

[features]
integration = []
stress = []

[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
tokio = { version = "1", features = ["full"] }
tempfile = "3"

[[bench]]
name = "<engine>_benchmarks"
harness = false
```

Add it to the workspace in the root `Cargo.toml`:

```toml
members = [
    "src-tauri",
    "crates/sakidb-core",
    "crates/sakidb-postgres",
    "crates/sakidb-sqlite",
    "crates/sakidb-<engine>",  # ← add here
]
```

**Rule:** The driver crate depends on `sakidb-core` and nothing else from the workspace. This is a compile-time guarantee — if it compiles, the boundary holds.

---

## Step 3: Implement traits

### Required: `Driver`

Every engine must implement `Driver`. This is the base trait for connection lifecycle.

```rust
use async_trait::async_trait;
use sakidb_core::{Driver, Result};
use sakidb_core::types::*;

pub struct YourDriver { /* connection pool, state, etc. */ }

#[async_trait]
impl Driver for YourDriver {
    fn engine_type(&self) -> EngineType {
        EngineType::YourEngine
    }

    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            // Trait-level: which optional traits did you implement?
            sql: true,
            introspection: true,
            export: false,
            restore: false,
            key_value: false,
            document: false,

            // Feature-level: what does your introspection support?
            schemas: false,        // true if engine has schema namespaces
            tables: true,
            views: false,
            materialized_views: false,
            functions: false,
            sequences: false,
            indexes: true,
            triggers: false,
            partitions: false,
            foreign_tables: false,
            explain: false,
            multi_database: false, // true if engine supports multiple databases
        }
    }

    async fn connect(&self, config: &ConnectionConfig) -> Result<ConnectionId> {
        // Create connection, return a ConnectionId
    }

    async fn disconnect(&self, conn_id: &ConnectionId) -> Result<()> {
        // Clean up connection
    }

    async fn test_connection(&self, config: &ConnectionConfig) -> Result<()> {
        // Connect, run a simple query, disconnect
    }
}
```

### Optional: `SqlDriver`

For SQL-based engines. Must implement `execute`, `execute_multi`, `execute_paged`, `execute_batch`, and `cancel_query`.

The `execute_multi_columnar` method has a default implementation that converts row-based results via `rows_to_columnar()`. Override it only if your driver can produce columnar data natively for better performance.

### Optional: `Introspector`

For engines that support schema introspection. Methods that your engine doesn't support should return `Ok(vec![])` or `Err(SakiError::NotSupported(...))`.

### Optional: `Exporter`, `Restorer`

For streaming data export and SQL file restore. Both support cancellation via `AtomicBool`.

### Optional: `KeyValueDriver`, `DocumentDriver`

For non-SQL engines (Redis, MongoDB). These are not yet wired into the frontend.

---

## Step 4: EngineCapabilities drives the UI

The `EngineCapabilities` struct you return from `capabilities()` directly controls what the frontend shows:

| Capability | Frontend behavior when `false` |
|---|---|
| `sql` | Hides query editor, "New Query" context menu |
| `introspection` | Hides schema tree, ERD |
| `export` | Hides "Export" context menu |
| `restore` | Hides "Restore" context menu |
| `schemas` | Hides schema selector, flattens tree |
| `multi_database` | Hides database selector, "Create/Drop/Rename DB" |
| `views` | Hides "Views" folder in tree |
| `functions` | Hides "Functions" folder |
| `explain` | Hides EXPLAIN buttons in query toolbar |
| `indexes` | Hides "Indexes" tab in structure view |
| `triggers` | Hides "Triggers" tab |
| `partitions` | Hides "Partitions" tab |

**You do not need to touch any frontend code.** Set capabilities correctly and the UI adapts.

---

## Step 5: Feature-flag in src-tauri

In `src-tauri/Cargo.toml`:

```toml
[dependencies]
sakidb-<engine> = { path = "../crates/sakidb-<engine>", optional = true }

[features]
default = ["postgres", "sqlite", "<engine>"]
<engine> = ["dep:sakidb-<engine>"]
```

---

## Step 6: Register in DriverRegistry

In `src-tauri/src/lib.rs`, add a registration block:

```rust
#[cfg(feature = "<engine>")]
{
    use sakidb_<engine>::YourDriver;
    let drv = Arc::new(YourDriver::new());
    registry.register(
        sakidb_core::types::EngineType::YourEngine,
        DriverEntry {
            driver: drv.clone(),
            sql: Some(drv.clone()),         // if SqlDriver implemented
            introspector: Some(drv.clone()), // if Introspector implemented
            exporter: None,                 // if not implemented
            restorer: None,
            key_value: None,
            document: None,
        },
    );
}
```

The `DriverEntry` fields must match what you declared in `EngineCapabilities`. If `capabilities().sql == true`, then `sql` must be `Some(...)`. The registry will return `NotSupported` errors if a caller requests a trait that isn't registered.

---

## Step 7: Engine-specific commands (optional)

If your engine has operations that don't fit any trait (e.g., SQLite's `VACUUM`, `PRAGMA integrity_check`), add a dedicated command module:

1. Create `src-tauri/src/commands/<engine>.rs`
2. Add the module to `src-tauri/src/commands/mod.rs`
3. Register commands in `invoke_handler` in `src-tauri/src/lib.rs`
4. Cast the driver: access engine-specific methods by downcasting or by importing the driver type directly behind `#[cfg(feature = "...")]`

Keep these minimal. If an operation can be expressed as a SQL query, use `SqlDriver::execute` instead.

---

## Step 8: Tests

Follow the conventions in `CONTRIBUTING.md`:

1. **Unit tests** — `crates/sakidb-<engine>/src/*_test.rs` files for each module
2. **Integration tests** — `crates/sakidb-<engine>/tests/integration.rs` behind `#![cfg(feature = "integration")]`
3. **Stress tests** — `crates/sakidb-<engine>/tests/stress.rs` behind `#![cfg(feature = "stress")]`
4. **Benchmarks** — `crates/sakidb-<engine>/benches/<engine>_benchmarks.rs`

Verify:
```bash
cargo test -p sakidb-<engine>                          # unit tests
cargo test -p sakidb-<engine> --features integration   # integration tests
cargo bench -p sakidb-<engine>                         # benchmarks
cargo test --workspace                                 # full workspace still passes
```

---

## Checklist

Before submitting your PR:

- [ ] `EngineType` variant added with `Display` and `FromStr`
- [ ] Crate added to workspace `members`
- [ ] `Driver` trait implemented with correct `EngineCapabilities`
- [ ] Optional traits implemented (at minimum `SqlDriver` + `Introspector` for a useful driver)
- [ ] Feature flag added in `src-tauri/Cargo.toml`
- [ ] Driver registered in `src-tauri/src/lib.rs`
- [ ] Unit tests in `_test.rs` files covering public API
- [ ] Integration tests behind `#![cfg(feature = "integration")]`
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` passes
- [ ] No changes to `sakidb-core` types (except `EngineType` variant)
- [ ] No changes to frontend code
