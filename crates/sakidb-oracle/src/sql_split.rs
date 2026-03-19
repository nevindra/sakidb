pub fn split_sql_statements(sql_content: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut string_delim = '\0';
    let mut line_comment = false;
    let mut block_comment = false;
    let mut begin_count = 0;
    let mut is_plsql = false;

    let chars: Vec<char> = sql_content.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        
        // Handle comments and strings
        if line_comment {
            if ch == '\n' {
                line_comment = false;
            }
            i += 1;
            continue;
        }

        if block_comment {
            if ch == '*' && i + 1 < chars.len() && chars[i+1] == '/' {
                block_comment = false;
                current.push('*');
                current.push('/');
                i += 2;
            } else {
                current.push(ch);
                i += 1;
            }
            continue;
        }

        if !in_string {
            if ch == '\'' || ch == '"' {
                in_string = true;
                string_delim = ch;
                current.push(ch);
                i += 1;
                continue;
            }
            if ch == '-' && i + 1 < chars.len() && chars[i+1] == '-' {
                line_comment = true;
                i += 2;
                continue;
            }
            if ch == '/' && i + 1 < chars.len() && chars[i+1] == '*' {
                // [Fix: Minor 3] Preserve Oracle hints (/*+ ... */) which can change query plans.
                // M: Check if it's an Oracle hint
                if i + 2 < chars.len() && chars[i+2] == '+' {
                    block_comment = true;
                    current.push('/');
                    current.push('*');
                    i += 2;
                    continue;
                } else {
                    // Regular block comment - skip
                    let mut j = i + 2;
                    while j + 1 < chars.len() {
                        if chars[j] == '*' && chars[j+1] == '/' {
                            i = j + 2;
                            // Add a space to avoid joining words
                            if !current.ends_with(' ') {
                                current.push(' ');
                            }
                            break;
                        }
                        j += 1;
                    }
                    if j + 1 >= chars.len() {
                        i = chars.len();
                    }
                    continue;
                }
            }
        } else if ch == string_delim {
            // Check for escaped quote (e.g. '')
            if ch == '\'' && i + 1 < chars.len() && chars[i+1] == '\'' {
                current.push('\'');
                current.push('\'');
                i += 2;
                continue;
            }
            in_string = false;
            string_delim = '\0';
            current.push(ch);
            i += 1;
            continue;
        }

        if in_string {
            current.push(ch);
            i += 1;
            continue;
        }

        // Better: look for BEGIN/END words
        let remaining = &chars[i..];
        if starts_with_word(remaining, "BEGIN") || starts_with_word(remaining, "DECLARE") {
            begin_count += 1;
            is_plsql = true;
            current.push_str(if starts_with_word(remaining, "BEGIN") { "BEGIN" } else { "DECLARE" });
            i += if starts_with_word(remaining, "BEGIN") { 5 } else { 7 };
            continue;
        } else if starts_with_word(remaining, "END") {
            if begin_count > 0 {
                begin_count -= 1;
            }
            current.push_str("END");
            i += 3;
            continue;
        }

        if ch == ';' && begin_count == 0 {
            if is_plsql {
                current.push(';');
            }
            let stmt = current.trim().to_string();
            if !stmt.is_empty() {
                statements.push(stmt);
            }
            current.clear();
            is_plsql = false;
        } else {
            current.push(ch);
        }

        i += 1;
    }

    let last = current.trim().to_string();
    if !last.is_empty() {
        statements.push(last);
    }

    statements
}

fn starts_with_word(chars: &[char], word: &str) -> bool {
    if chars.len() < word.len() {
        return false;
    }
    for (i, c) in word.chars().enumerate() {
        if chars[i].to_uppercase().next() != c.to_uppercase().next() {
            return false;
        }
    }
    // Check if it's a whole word
    if chars.len() > word.len() {
        let next_char = chars[word.len()];
        if next_char.is_alphanumeric() || next_char == '_' {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::split_sql_statements;

    #[test]
    fn test_simple_split() {
        let sql = "SELECT * FROM t1; SELECT * FROM t2;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT * FROM t1");
        assert_eq!(stmts[1], "SELECT * FROM t2");
    }

    #[test]
    fn test_quoted_semicolon() {
        let sql = "INSERT INTO t1 (c1) VALUES ('a;b'); SELECT 1;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "INSERT INTO t1 (c1) VALUES ('a;b')");
        assert_eq!(stmts[1], "SELECT 1");
    }

    #[test]
    fn test_escaped_quote() {
        let sql = "INSERT INTO t1 (c1) VALUES ('O''Reilly; and more'); SELECT 1;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "INSERT INTO t1 (c1) VALUES ('O''Reilly; and more')");
        assert_eq!(stmts[1], "SELECT 1");
    }

    #[test]
    fn test_comments() {
        let sql = "SELECT 1; -- comment with ; \n SELECT 2; /* block \n with ; */ SELECT 3;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 3);
        assert_eq!(stmts[0], "SELECT 1");
        assert_eq!(stmts[1], "SELECT 2");
        assert_eq!(stmts[2], "SELECT 3");
    }

    #[test]
    fn test_oracle_hints() {
        let sql = "SELECT /*+ FULL(t) */ * FROM t; SELECT /* regular comment */ 1 FROM dual;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT /*+ FULL(t) */ * FROM t");
        assert_eq!(stmts[1], "SELECT  1 FROM dual");
    }

    #[test]
    fn test_plsql_block() {
        let sql = "
            DECLARE
              v_count NUMBER;
            BEGIN
              SELECT COUNT(*) INTO v_count FROM users;
              IF v_count > 0 THEN
                NULL;
              END IF;
            END;
            SELECT 1 FROM dual;
        ";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("END;"));
        // PL/SQL blocks within DECLARE/BEGIN/END keep their internal semicolons
        assert!(stmts[0].contains("v_count NUMBER;"));
        assert_eq!(stmts[1], "SELECT 1 FROM dual");
    }

    #[test]
    fn test_nested_plsql() {
        let sql = "
            BEGIN
              BEGIN
                NULL;
              END;
            END;
        ";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("END;"));
    }

    #[test]
    fn test_no_trailing_semicolon() {
        let sql = "SELECT 1; SELECT 2";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT 1");
        assert_eq!(stmts[1], "SELECT 2");
    }
}
