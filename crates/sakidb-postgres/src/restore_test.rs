use crate::restore::{extract_dollar_tag_from_bytes, is_copy_from_stdin, SqlParser};

/// Convenience wrapper for tests — extract dollar tag from a &str.
fn extract_dollar_tag(s: &str) -> Option<(String, usize)> {
    extract_dollar_tag_from_bytes(s.as_bytes())
}

#[test]
fn test_simple_statements() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("SELECT 1; SELECT 2;");
    assert_eq!(stmts, vec!["SELECT 1;", "SELECT 2;"]);
}

#[test]
fn test_multiline_statement() {
    let mut parser = SqlParser::new();
    assert!(parser.feed_line("SELECT").is_empty());
    let stmts = parser.feed_line("  1;");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("SELECT"));
    assert!(stmts[0].contains("1;"));
}

#[test]
fn test_single_quote_string() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("SELECT 'hello; world';");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("hello; world"));
}

#[test]
fn test_escaped_quote() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("SELECT 'it''s';");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("it''s"));
}

#[test]
fn test_dollar_quote() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("SELECT $$ hello; world $$;");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("hello; world"));
}

#[test]
fn test_dollar_quote_with_tag() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("SELECT $fn$ hello; world $fn$;");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("hello; world"));
}

#[test]
fn test_line_comment() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("-- this is a comment");
    assert!(stmts.is_empty());
}

#[test]
fn test_inline_comment() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("SELECT 1; -- comment");
    assert_eq!(stmts.len(), 1);
    assert_eq!(stmts[0], "SELECT 1;");
}

#[test]
fn test_block_comment() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("SELECT /* skip ; this */ 1;");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("1;"));
    assert!(!stmts[0].contains("skip"));
}

#[test]
fn test_multiline_block_comment() {
    let mut parser = SqlParser::new();
    assert!(parser.feed_line("/* start").is_empty());
    assert!(parser.feed_line("middle").is_empty());
    let stmts = parser.feed_line("end */ SELECT 1;");
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_psql_meta_command_skipped() {
    let mut parser = SqlParser::new();
    // psql meta-commands like \restrict should be skipped
    assert!(parser
        .feed_line("\\restrict NvLVw192Nr3zeflW9aQYuZ2Y")
        .is_empty());
    // Next statement should parse normally
    let stmts = parser.feed_line("SET statement_timeout = 0;");
    assert_eq!(stmts.len(), 1);
    assert_eq!(stmts[0], "SET statement_timeout = 0;");
}

#[test]
fn test_backslash_inside_string_not_skipped() {
    let mut parser = SqlParser::new();
    // Backslash inside a multi-line string should NOT skip the line
    parser.in_single_quote = true;
    let stmts = parser.feed_line("\\some data');");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("\\some data"));
}

#[test]
fn test_double_quoted_identifier_with_semicolon() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("CREATE TABLE \"my;table\" (id int);");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("\"my;table\""));
}

#[test]
fn test_double_quoted_escaped_quote() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("SELECT \"col\"\"name\" FROM t;");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("\"col\"\"name\""));
}

#[test]
fn test_copy_detection() {
    assert!(is_copy_from_stdin(
        "COPY public.users (id, name) FROM stdin;"
    ));
    assert!(is_copy_from_stdin("COPY users FROM STDIN;"));
    assert!(!is_copy_from_stdin("COPY users TO stdout;"));
    assert!(!is_copy_from_stdin("SELECT 1;"));
}

#[test]
fn test_dollar_tag_extraction() {
    assert_eq!(extract_dollar_tag("$$"), Some(("".to_string(), 2)));
    assert_eq!(extract_dollar_tag("$fn$"), Some(("fn".to_string(), 4)));
    assert_eq!(
        extract_dollar_tag("$body$"),
        Some(("body".to_string(), 6))
    );
    assert_eq!(extract_dollar_tag("$123"), None); // tag can't start with digit
    assert_eq!(extract_dollar_tag("abc"), None);
}

// ── Additional is_copy_from_stdin tests ──

#[test]
fn test_copy_from_stdin_case_insensitive() {
    assert!(is_copy_from_stdin("copy users from stdin;"));
    assert!(is_copy_from_stdin("COPY users FROM STDIN;"));
    assert!(is_copy_from_stdin("Copy Users From Stdin;"));
    assert!(is_copy_from_stdin("cOpY users fRoM sTdIn;"));
}

#[test]
fn test_copy_from_stdin_with_columns() {
    assert!(is_copy_from_stdin(
        "COPY public.users (id, name, email) FROM stdin WITH (FORMAT csv);"
    ));
}

#[test]
fn test_copy_from_stdin_short_strings() {
    // Strings shorter than 5 bytes
    assert!(!is_copy_from_stdin(""));
    assert!(!is_copy_from_stdin("COPY"));
    assert!(!is_copy_from_stdin("COP"));
    assert!(!is_copy_from_stdin("X"));
}

#[test]
fn test_copy_to_stdout_rejected() {
    assert!(!is_copy_from_stdin("COPY users TO stdout;"));
    assert!(!is_copy_from_stdin("COPY users TO STDOUT;"));
    assert!(!is_copy_from_stdin("copy users to stdout;"));
}

#[test]
fn test_copy_not_copy() {
    assert!(!is_copy_from_stdin("SELECT * FROM users;"));
    assert!(!is_copy_from_stdin("INSERT INTO users VALUES (1);"));
    assert!(!is_copy_from_stdin("CREATE TABLE copy_test (id int);"));
}

// ── Additional extract_dollar_tag tests ──

#[test]
fn test_dollar_tag_underscore() {
    assert_eq!(
        extract_dollar_tag("$_tag$"),
        Some(("_tag".to_string(), 6))
    );
    assert_eq!(
        extract_dollar_tag("$_$"),
        Some(("_".to_string(), 3))
    );
}

#[test]
fn test_dollar_tag_alphanumeric() {
    assert_eq!(
        extract_dollar_tag("$abc123$"),
        Some(("abc123".to_string(), 8))
    );
    assert_eq!(
        extract_dollar_tag("$a1_b2$"),
        Some(("a1_b2".to_string(), 7))
    );
}

#[test]
fn test_dollar_tag_empty_input() {
    assert_eq!(extract_dollar_tag(""), None);
}

#[test]
fn test_dollar_tag_no_closing_dollar() {
    assert_eq!(extract_dollar_tag("$abc"), None);
    assert_eq!(extract_dollar_tag("$"), None);
}

#[test]
fn test_dollar_tag_invalid_start_char() {
    // Tag can't start with a digit
    assert_eq!(extract_dollar_tag("$1abc$"), None);
    // Tag can't contain special chars
    assert_eq!(extract_dollar_tag("$a-b$"), None);
    assert_eq!(extract_dollar_tag("$a.b$"), None);
}

#[test]
fn test_dollar_tag_not_starting_with_dollar() {
    assert_eq!(extract_dollar_tag("abc$"), None);
    assert_eq!(extract_dollar_tag("x$$"), None);
}

// ── Additional SqlParser tests ──

#[test]
fn test_parser_empty_input() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("");
    assert!(stmts.is_empty());
}

#[test]
fn test_parser_whitespace_only() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("   \t  ");
    assert!(stmts.is_empty());
}

#[test]
fn test_parser_multiple_statements_one_line() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("SELECT 1; SELECT 2; SELECT 3;");
    assert_eq!(stmts.len(), 3);
    assert_eq!(stmts[0], "SELECT 1;");
    assert_eq!(stmts[1], "SELECT 2;");
    assert_eq!(stmts[2], "SELECT 3;");
}

#[test]
fn test_parser_multiline_dollar_quote() {
    let mut parser = SqlParser::new();
    assert!(parser.feed_line("CREATE FUNCTION f() AS $$").is_empty());
    assert!(parser.feed_line("BEGIN").is_empty());
    assert!(parser.feed_line("  RETURN 1;").is_empty());
    assert!(parser.feed_line("END;").is_empty());
    let stmts = parser.feed_line("$$;");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("CREATE FUNCTION"));
    assert!(stmts[0].contains("RETURN 1;"));
}

#[test]
fn test_parser_nested_block_comments() {
    let mut parser = SqlParser::new();
    let stmts = parser.feed_line("SELECT /* outer /* inner */ outer */ 1;");
    assert_eq!(stmts.len(), 1);
    // The block comment content should be stripped
    assert!(stmts[0].contains("1;"));
    assert!(!stmts[0].contains("inner"));
}

#[test]
fn test_parser_multiline_block_comment_with_semicolons() {
    let mut parser = SqlParser::new();
    assert!(parser.feed_line("/* comment with ; ").is_empty());
    assert!(parser.feed_line("more ; stuff").is_empty());
    let stmts = parser.feed_line("*/ SELECT 1;");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("SELECT 1;"));
}

#[test]
fn test_parser_initial_state() {
    let parser = SqlParser::new();
    assert!(parser.buf.is_empty());
    assert!(!parser.in_single_quote);
    assert!(!parser.in_double_quote);
    assert!(parser.dollar_quote_tag.is_none());
    assert_eq!(parser.block_comment_depth, 0);
}

#[test]
fn test_parser_single_quote_spanning_lines() {
    let mut parser = SqlParser::new();
    assert!(parser.feed_line("SELECT 'hello ").is_empty());
    let stmts = parser.feed_line("world';");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("hello"));
    assert!(stmts[0].contains("world"));
}

#[test]
fn test_parser_double_quote_spanning_lines() {
    let mut parser = SqlParser::new();
    assert!(parser.feed_line("SELECT \"col ").is_empty());
    let stmts = parser.feed_line("name\" FROM t;");
    assert_eq!(stmts.len(), 1);
    assert!(stmts[0].contains("\"col"));
    assert!(stmts[0].contains("name\""));
}

// ── RestoreProgress tests ──

#[test]
fn test_restore_progress_default_values() {
    let progress = sakidb_core::types::RestoreProgress {
        bytes_read: 0,
        total_bytes: 0,
        statements_executed: 0,
        errors_skipped: 0,
        phase: "Starting".to_string(),
        elapsed_ms: 0,
        error: None,
        error_messages: Vec::new(),
    };
    assert_eq!(progress.bytes_read, 0);
    assert_eq!(progress.total_bytes, 0);
    assert_eq!(progress.statements_executed, 0);
    assert_eq!(progress.errors_skipped, 0);
    assert_eq!(progress.phase, "Starting");
    assert!(progress.error.is_none());
    assert!(progress.error_messages.is_empty());
}

#[test]
fn test_restore_progress_json_serialization() {
    let progress = sakidb_core::types::RestoreProgress {
        bytes_read: 1024,
        total_bytes: 4096,
        statements_executed: 10,
        errors_skipped: 2,
        phase: "Executing".to_string(),
        elapsed_ms: 500,
        error: Some("test error".to_string()),
        error_messages: vec!["err1".to_string(), "err2".to_string()],
    };
    let json = serde_json::to_string(&progress).unwrap();
    assert!(json.contains("\"bytes_read\":1024"));
    assert!(json.contains("\"total_bytes\":4096"));
    assert!(json.contains("\"statements_executed\":10"));
    assert!(json.contains("\"errors_skipped\":2"));
    assert!(json.contains("\"phase\":\"Executing\""));
    assert!(json.contains("\"elapsed_ms\":500"));
    assert!(json.contains("\"error\":\"test error\""));
}
