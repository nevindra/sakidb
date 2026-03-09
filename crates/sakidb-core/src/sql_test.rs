use super::sql::*;

// ── One-shot splitter tests ──

#[test]
fn test_split_simple() {
    let stmts = split_sql_statements("SELECT 1; SELECT 2;");
    assert_eq!(stmts, vec!["SELECT 1", "SELECT 2"]);
}

#[test]
fn test_split_no_trailing_semi() {
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
fn test_split_line_comment() {
    let stmts = split_sql_statements("SELECT 1; -- comment;\nSELECT 2");
    assert_eq!(stmts, vec!["SELECT 1", "-- comment;\nSELECT 2"]);
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

#[test]
fn test_split_double_quoted_identifier() {
    let stmts = split_sql_statements("SELECT \"col;name\" FROM t; SELECT 2");
    assert_eq!(stmts, vec!["SELECT \"col;name\" FROM t", "SELECT 2"]);
}

// ── Dollar-quoting tests ──

#[test]
fn test_split_dollar_quoting() {
    let opts = SqlSplitOptions { dollar_quoting: true };
    let stmts = split_sql_statements_with(
        "SELECT $$hello;world$$; SELECT 2",
        &opts,
    );
    assert_eq!(stmts, vec!["SELECT $$hello;world$$", "SELECT 2"]);
}

#[test]
fn test_split_dollar_tagged() {
    let opts = SqlSplitOptions { dollar_quoting: true };
    let stmts = split_sql_statements_with(
        "CREATE FUNCTION f() RETURNS void AS $body$BEGIN; END;$body$; SELECT 1",
        &opts,
    );
    assert_eq!(
        stmts,
        vec![
            "CREATE FUNCTION f() RETURNS void AS $body$BEGIN; END;$body$",
            "SELECT 1"
        ]
    );
}

// ── Streaming splitter tests ──

#[test]
fn test_streaming_basic() {
    let mut sp = StreamingSqlSplitter::new(SqlSplitOptions::default());
    let s1 = sp.feed("SELECT 1; SELECT 2;");
    assert_eq!(s1, vec!["SELECT 1", "SELECT 2"]);
    assert_eq!(sp.finish(), None);
}

#[test]
fn test_streaming_across_chunks() {
    let mut sp = StreamingSqlSplitter::new(SqlSplitOptions::default());
    let s1 = sp.feed("SELECT ");
    assert!(s1.is_empty());
    let s2 = sp.feed("1; SEL");
    assert_eq!(s2, vec!["SELECT 1"]);
    let s3 = sp.feed("ECT 2");
    assert!(s3.is_empty());
    assert_eq!(sp.finish(), Some("SELECT 2".to_string()));
}

#[test]
fn test_streaming_string_across_chunks() {
    let mut sp = StreamingSqlSplitter::new(SqlSplitOptions::default());
    let s1 = sp.feed("SELECT 'hel");
    assert!(s1.is_empty());
    let s2 = sp.feed("lo;wor");
    assert!(s2.is_empty());
    let s3 = sp.feed("ld'; SELECT 2;");
    assert_eq!(s3, vec!["SELECT 'hello;world'", "SELECT 2"]);
}

#[test]
fn test_streaming_dollar_quoting() {
    let mut sp = StreamingSqlSplitter::new(SqlSplitOptions { dollar_quoting: true });
    let s1 = sp.feed("SELECT $$he");
    assert!(s1.is_empty());
    let s2 = sp.feed("llo;world$$; SELECT 2;");
    assert_eq!(s2, vec!["SELECT $$hello;world$$", "SELECT 2"]);
}

#[test]
fn test_streaming_block_comment_across_chunks() {
    let mut sp = StreamingSqlSplitter::new(SqlSplitOptions::default());
    let s1 = sp.feed("SELECT /* com");
    assert!(s1.is_empty());
    let s2 = sp.feed("ment; */ 1; SELECT 2;");
    assert_eq!(s2, vec!["SELECT /* comment; */ 1", "SELECT 2"]);
}

#[test]
fn test_streaming_utf8_preserved() {
    // Multi-byte UTF-8: é (U+00E9) = 0xC3 0xA9, ñ (U+00F1) = 0xC3 0xB1
    let mut sp = StreamingSqlSplitter::new(SqlSplitOptions::default());
    let s1 = sp.feed("SELECT 'café'; INSERT INTO t VALUES ('señor');");
    assert_eq!(s1, vec!["SELECT 'café'", "INSERT INTO t VALUES ('señor')"]);
}

#[test]
fn test_streaming_utf8_across_chunks() {
    // Split a chunk in the middle of normal text containing multi-byte chars
    let mut sp = StreamingSqlSplitter::new(SqlSplitOptions::default());
    let s1 = sp.feed("SELECT 'caf");
    assert!(s1.is_empty());
    let s2 = sp.feed("é'; SELECT '日本語';");
    assert_eq!(s2, vec!["SELECT 'café'", "SELECT '日本語'"]);
}

#[test]
fn test_streaming_utf8_in_identifiers() {
    let mut sp = StreamingSqlSplitter::new(SqlSplitOptions::default());
    let s1 = sp.feed("SELECT * FROM \"таблица\"; SELECT 1;");
    assert_eq!(s1, vec!["SELECT * FROM \"таблица\"", "SELECT 1"]);
}
