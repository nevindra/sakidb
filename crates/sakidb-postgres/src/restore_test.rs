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
