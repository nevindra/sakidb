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
