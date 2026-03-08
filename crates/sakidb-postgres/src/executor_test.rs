use crate::executor::split_sql_statements;

#[test]
fn test_split_simple() {
    let stmts = split_sql_statements("SELECT 1; SELECT 2;");
    assert_eq!(stmts, vec!["SELECT 1", "SELECT 2"]);
}

#[test]
fn test_split_no_trailing_semicolon() {
    let stmts = split_sql_statements("SELECT 1; SELECT 2");
    assert_eq!(stmts, vec!["SELECT 1", "SELECT 2"]);
}

#[test]
fn test_split_string_literal() {
    let stmts = split_sql_statements("SELECT 'hello;world'; SELECT 2");
    assert_eq!(stmts, vec!["SELECT 'hello;world'", "SELECT 2"]);
}

#[test]
fn test_split_escaped_quote() {
    let stmts = split_sql_statements("SELECT 'it''s'; SELECT 2");
    assert_eq!(stmts, vec!["SELECT 'it''s'", "SELECT 2"]);
}

#[test]
fn test_split_dollar_quoted() {
    let stmts = split_sql_statements("SELECT $$hello;world$$; SELECT 2");
    assert_eq!(stmts, vec!["SELECT $$hello;world$$", "SELECT 2"]);
}

#[test]
fn test_split_tagged_dollar_quote() {
    let stmts = split_sql_statements("CREATE FUNCTION f() RETURNS void AS $body$BEGIN; END;$body$; SELECT 1");
    assert_eq!(stmts, vec!["CREATE FUNCTION f() RETURNS void AS $body$BEGIN; END;$body$", "SELECT 1"]);
}

#[test]
fn test_split_line_comment() {
    let stmts = split_sql_statements("SELECT 1; -- this is a comment;\nSELECT 2");
    assert_eq!(stmts, vec!["SELECT 1", "-- this is a comment;\nSELECT 2"]);
}

#[test]
fn test_split_block_comment() {
    let stmts = split_sql_statements("SELECT /* ; */ 1; SELECT 2");
    assert_eq!(stmts, vec!["SELECT /* ; */ 1", "SELECT 2"]);
}

#[test]
fn test_split_empty() {
    let stmts = split_sql_statements("   ;  ;  ");
    assert!(stmts.is_empty());
}

#[test]
fn test_split_single() {
    let stmts = split_sql_statements("SELECT 1");
    assert_eq!(stmts, vec!["SELECT 1"]);
}

// ── Additional split_sql edge cases ──

#[test]
fn test_split_nested_block_comments() {
    let stmts = split_sql_statements("SELECT /* outer /* inner */ still comment */ 1; SELECT 2");
    assert_eq!(stmts, vec!["SELECT /* outer /* inner */ still comment */ 1", "SELECT 2"]);
}

#[test]
fn test_split_multiple_semicolons() {
    let stmts = split_sql_statements("SELECT 1;;; SELECT 2");
    // Empty statements between semicolons should be filtered out
    assert_eq!(stmts, vec!["SELECT 1", "SELECT 2"]);
}

#[test]
fn test_split_whitespace_between() {
    let stmts = split_sql_statements("  SELECT 1  ;  SELECT 2  ;  ");
    assert_eq!(stmts, vec!["SELECT 1", "SELECT 2"]);
}

#[test]
fn test_split_complex_plpgsql() {
    let sql = "CREATE OR REPLACE FUNCTION inc(val int) RETURNS int AS $fn$\nBEGIN\n  RETURN val + 1;\nEND;\n$fn$ LANGUAGE plpgsql; SELECT inc(1)";
    let stmts = split_sql_statements(sql);
    assert_eq!(stmts.len(), 2);
    assert!(stmts[0].contains("$fn$"));
    assert!(stmts[0].contains("RETURN val + 1;"));
    assert_eq!(stmts[1], "SELECT inc(1)");
}

#[test]
fn test_split_multiline_string() {
    let sql = "INSERT INTO t VALUES ('line1\nline2\nline3'); SELECT 1";
    let stmts = split_sql_statements(sql);
    assert_eq!(stmts.len(), 2);
    assert!(stmts[0].contains("line1\nline2\nline3"));
    assert_eq!(stmts[1], "SELECT 1");
}

#[test]
fn test_split_double_quoted_identifier() {
    let stmts = split_sql_statements("SELECT \"col;name\" FROM t; SELECT 2");
    assert_eq!(stmts.len(), 2);
    assert!(stmts[0].contains("\"col;name\""));
}

#[test]
fn test_split_mixed_comments_and_strings() {
    let sql = "SELECT 'hello' /* comment ; */ -- line;\n; SELECT 2";
    let stmts = split_sql_statements(sql);
    assert_eq!(stmts.len(), 2);
}

#[test]
fn test_split_only_comments() {
    let stmts = split_sql_statements("-- just a comment\n/* block */");
    // Comments alone with no real statements
    assert!(stmts.is_empty() || stmts.iter().all(|s| s.trim().starts_with("--") || s.trim().starts_with("/*")));
}

#[test]
fn test_split_dollar_quote_with_underscore_tag() {
    let stmts = split_sql_statements("SELECT $my_tag$hello;world$my_tag$; SELECT 2");
    assert_eq!(stmts.len(), 2);
    assert!(stmts[0].contains("hello;world"));
}

#[test]
fn test_split_adjacent_dollar_quotes() {
    let stmts = split_sql_statements("SELECT $$a$$; SELECT $$b$$");
    assert_eq!(stmts.len(), 2);
}
