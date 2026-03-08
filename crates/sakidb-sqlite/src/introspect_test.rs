use rusqlite::Connection;

// ── list_tables tests ──

#[test]
fn test_list_tables_empty_db() {
    let conn = Connection::open_in_memory().unwrap();
    let tables = crate::introspect::list_tables(&conn).unwrap();
    assert!(tables.is_empty());
}

#[test]
fn test_list_tables_with_tables() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER);"
    ).unwrap();
    let tables = crate::introspect::list_tables(&conn).unwrap();
    assert_eq!(tables.len(), 2);
    // Tables should be sorted by name
    assert_eq!(tables[0].name, "posts");
    assert_eq!(tables[1].name, "users");
}

#[test]
fn test_list_tables_excludes_sqlite_internal() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE users (id INTEGER PRIMARY KEY)").unwrap();
    // sqlite_master, sqlite_sequence, etc. should not appear
    let tables = crate::introspect::list_tables(&conn).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "users");
}

#[test]
fn test_list_tables_partition_defaults() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER)").unwrap();
    let tables = crate::introspect::list_tables(&conn).unwrap();
    assert!(!tables[0].is_partition);
    assert!(tables[0].parent_table.is_none());
    assert!(tables[0].row_count_estimate.is_none());
    assert!(tables[0].size_bytes.is_none());
}

// ── list_columns tests ──

#[test]
fn test_list_columns_basic() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT DEFAULT 'unknown')"
    ).unwrap();
    let columns = crate::introspect::list_columns(&conn, "users").unwrap();
    assert_eq!(columns.len(), 3);

    assert_eq!(columns[0].name, "id");
    assert!(columns[0].is_primary_key);

    assert_eq!(columns[1].name, "name");
    assert!(!columns[1].is_nullable); // NOT NULL

    assert_eq!(columns[2].name, "email");
    assert!(columns[2].is_nullable);
    assert_eq!(columns[2].default_value.as_deref(), Some("'unknown'"));
}

#[test]
fn test_list_columns_empty_table() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE empty_t (a INTEGER, b TEXT, c REAL)").unwrap();
    let columns = crate::introspect::list_columns(&conn, "empty_t").unwrap();
    assert_eq!(columns.len(), 3);
    assert_eq!(columns[0].name, "a");
    assert_eq!(columns[1].name, "b");
    assert_eq!(columns[2].name, "c");
}

#[test]
fn test_list_columns_quoted_table_name() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE \"my table\" (id INTEGER)").unwrap();
    let columns = crate::introspect::list_columns(&conn, "my table").unwrap();
    assert_eq!(columns.len(), 1);
    assert_eq!(columns[0].name, "id");
}

// ── list_views tests ──

#[test]
fn test_list_views_empty() {
    let conn = Connection::open_in_memory().unwrap();
    let views = crate::introspect::list_views(&conn).unwrap();
    assert!(views.is_empty());
}

#[test]
fn test_list_views_with_views() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER, name TEXT);
         CREATE VIEW active_users AS SELECT * FROM users;
         CREATE VIEW admin_users AS SELECT * FROM users WHERE id = 1;"
    ).unwrap();
    let views = crate::introspect::list_views(&conn).unwrap();
    assert_eq!(views.len(), 2);
    assert_eq!(views[0].name, "active_users");
    assert_eq!(views[1].name, "admin_users");
    // SQLite views are not updatable
    assert!(!views[0].is_updatable);
}

// ── list_indexes tests ──

#[test]
fn test_list_indexes_no_indexes() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER, name TEXT)").unwrap();
    let indexes = crate::introspect::list_indexes(&conn, "t").unwrap();
    assert!(indexes.is_empty());
}

#[test]
fn test_list_indexes_with_index() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER, name TEXT);
         CREATE INDEX idx_name ON users(name);"
    ).unwrap();
    let indexes = crate::introspect::list_indexes(&conn, "users").unwrap();
    assert_eq!(indexes.len(), 1);
    assert_eq!(indexes[0].name, "idx_name");
    assert_eq!(indexes[0].table_name, "users");
    assert!(!indexes[0].is_unique);
    assert!(!indexes[0].is_primary);
}

#[test]
fn test_list_indexes_unique() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER, email TEXT);
         CREATE UNIQUE INDEX idx_email ON users(email);"
    ).unwrap();
    let indexes = crate::introspect::list_indexes(&conn, "users").unwrap();
    assert_eq!(indexes.len(), 1);
    assert!(indexes[0].is_unique);
}

// ── list_all_indexes tests ──

#[test]
fn test_list_all_indexes() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t1 (id INTEGER, val TEXT);
         CREATE TABLE t2 (id INTEGER, val TEXT);
         CREATE INDEX idx_t1 ON t1(val);
         CREATE INDEX idx_t2 ON t2(val);"
    ).unwrap();
    let indexes = crate::introspect::list_all_indexes(&conn).unwrap();
    assert_eq!(indexes.len(), 2);
}

// ── list_triggers tests ──

#[test]
fn test_list_triggers_empty() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER)").unwrap();
    let triggers = crate::introspect::list_triggers(&conn, "t").unwrap();
    assert!(triggers.is_empty());
}

#[test]
fn test_list_triggers_with_trigger() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t (id INTEGER, updated_at TEXT);
         CREATE TABLE log (msg TEXT);
         CREATE TRIGGER trg_after_insert AFTER INSERT ON t
         BEGIN INSERT INTO log VALUES ('inserted'); END;"
    ).unwrap();
    let triggers = crate::introspect::list_triggers(&conn, "t").unwrap();
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0].name, "trg_after_insert");
    assert_eq!(triggers[0].table_name, "t");
    assert_eq!(triggers[0].timing, "AFTER");
    assert_eq!(triggers[0].event, "INSERT");
}

#[test]
fn test_list_triggers_before_delete() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t (id INTEGER, name TEXT);
         CREATE TABLE log (msg TEXT);
         CREATE TRIGGER trg_before_delete BEFORE DELETE ON t
         BEGIN SELECT 1; END;"
    ).unwrap();
    let triggers = crate::introspect::list_triggers(&conn, "t").unwrap();
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0].timing, "BEFORE");
    assert_eq!(triggers[0].event, "DELETE");
}

// ── list_foreign_keys tests ──

#[test]
fn test_list_foreign_keys_empty() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER)").unwrap();
    let fks = crate::introspect::list_foreign_keys(&conn, "t").unwrap();
    assert!(fks.is_empty());
}

#[test]
fn test_list_foreign_keys_basic() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER PRIMARY KEY);
         CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER REFERENCES users(id));"
    ).unwrap();
    let fks = crate::introspect::list_foreign_keys(&conn, "posts").unwrap();
    assert_eq!(fks.len(), 1);
    assert_eq!(fks[0].foreign_table_name, "users");
    assert_eq!(fks[0].columns, vec!["user_id"]);
    assert_eq!(fks[0].foreign_columns, vec!["id"]);
}

// ── list_check_constraints tests ──

#[test]
fn test_list_check_constraints_empty() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER)").unwrap();
    let checks = crate::introspect::list_check_constraints(&conn, "t").unwrap();
    assert!(checks.is_empty());
}

#[test]
fn test_list_check_constraints_with_constraint() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t (id INTEGER, age INTEGER CHECK(age >= 0))"
    ).unwrap();
    let checks = crate::introspect::list_check_constraints(&conn, "t").unwrap();
    assert_eq!(checks.len(), 1);
    assert!(checks[0].check_clause.contains("age >= 0"));
}

// ── list_unique_constraints tests ──

#[test]
fn test_list_unique_constraints_empty() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE t (id INTEGER, name TEXT)").unwrap();
    let uniques = crate::introspect::list_unique_constraints(&conn, "t").unwrap();
    assert!(uniques.is_empty());
}

#[test]
fn test_list_unique_constraints_with_unique() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t (id INTEGER, email TEXT);
         CREATE UNIQUE INDEX idx_email ON t(email);"
    ).unwrap();
    let uniques = crate::introspect::list_unique_constraints(&conn, "t").unwrap();
    assert_eq!(uniques.len(), 1);
    assert!(!uniques[0].is_primary);
}

// ── get_create_table_sql tests ──

#[test]
fn test_get_create_table_sql() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);"
    ).unwrap();
    let ddl = crate::introspect::get_create_table_sql(&conn, "users").unwrap();
    assert!(ddl.contains("CREATE TABLE"));
    assert!(ddl.contains("users"));
    assert!(ddl.contains("id"));
    assert!(ddl.contains("name"));
}

#[test]
fn test_get_create_table_sql_with_indexes() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER, name TEXT);
         CREATE INDEX idx_name ON users(name);"
    ).unwrap();
    let ddl = crate::introspect::get_create_table_sql(&conn, "users").unwrap();
    assert!(ddl.contains("CREATE TABLE"));
    assert!(ddl.contains("idx_name"));
}

// ── get_erd_data tests ──

#[test]
fn test_get_erd_data_empty() {
    let conn = Connection::open_in_memory().unwrap();
    let erd = crate::introspect::get_erd_data(&conn).unwrap();
    assert!(erd.tables.is_empty());
    assert!(erd.columns.is_empty());
    assert!(erd.foreign_keys.is_empty());
}

#[test]
fn test_get_erd_data_with_relations() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER REFERENCES users(id));"
    ).unwrap();
    let erd = crate::introspect::get_erd_data(&conn).unwrap();
    assert_eq!(erd.tables.len(), 2);
    assert_eq!(erd.columns.len(), 2);
    assert!(erd.columns.contains_key("users"));
    assert!(erd.columns.contains_key("posts"));
    // posts has a foreign key to users
    assert!(erd.foreign_keys.contains_key("posts"));
}

// ── get_schema_completion_data tests ──

#[test]
fn test_get_schema_completion_data() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER, name TEXT, email TEXT);"
    ).unwrap();
    let data = crate::introspect::get_schema_completion_data(&conn).unwrap();
    assert!(data.contains_key("users"));
    let cols = &data["users"];
    assert_eq!(cols.len(), 3);
    assert!(cols.contains(&"id".to_string()));
    assert!(cols.contains(&"name".to_string()));
    assert!(cols.contains(&"email".to_string()));
}

// ── get_completion_bundle tests ──

#[test]
fn test_get_completion_bundle() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER);
         CREATE VIEW v AS SELECT * FROM users;"
    ).unwrap();
    let bundle = crate::introspect::get_completion_bundle(&conn).unwrap();
    assert_eq!(bundle.tables.len(), 2); // table + view
    assert!(bundle.functions.is_empty()); // SQLite doesn't expose functions
}

// ── get_table_columns_for_completion tests ──

#[test]
fn test_get_table_columns_for_completion() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT)"
    ).unwrap();
    let cols = crate::introspect::get_table_columns_for_completion(&conn, "users").unwrap();
    assert_eq!(cols.len(), 3);
    assert_eq!(cols[0].name, "id");
    assert!(cols[0].is_primary_key);
    assert_eq!(cols[1].name, "name");
    assert!(!cols[1].is_nullable);
    assert_eq!(cols[2].name, "email");
    assert!(cols[2].is_nullable);
}
