pub fn split_sql_statements(sql_content: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut string_delim = '\0';
    let mut line_comment = false;
    let mut block_comment = false;
    let mut begin_count = 0;

    let chars: Vec<char> = sql_content.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        
        // Handle comments and strings
        if !line_comment && !block_comment {
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
                    current.push('-');
                    current.push('-');
                    i += 2;
                    continue;
                }
                if ch == '/' && i + 1 < chars.len() && chars[i+1] == '*' {
                    block_comment = true;
                    current.push('/');
                    current.push('*');
                    i += 2;
                    continue;
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
        } else if line_comment {
            if ch == '\n' {
                line_comment = false;
                current.push(ch);
            }
            i += 1;
            continue;
        } else if block_comment {
            if ch == '*' && i + 1 < chars.len() && chars[i+1] == '/' {
                block_comment = false;
                current.push('*');
                current.push('/');
                i += 2;
                continue;
            }
            current.push(ch);
            i += 1;
            continue;
        }

        if in_string || line_comment || block_comment {
            current.push(ch);
            i += 1;
            continue;
        }

        // Better: look for BEGIN/END words
        if !in_string && !line_comment && !block_comment {
            let remaining = &chars[i..];
            if starts_with_word(remaining, "BEGIN") || starts_with_word(remaining, "DECLARE") {
                begin_count += 1;
            } else if starts_with_word(remaining, "END")
                && begin_count > 0 {
                    begin_count -= 1;
                }
        }

        if ch == ';' && begin_count == 0 {
            current.push(ch);
            let stmt = current.trim().to_string();
            if !stmt.is_empty() && stmt != ";" {
                statements.push(stmt);
            }
            current.clear();
        } else {
            current.push(ch);
        }

        i += 1;
    }

    let last = current.trim().to_string();
    if !last.is_empty() && last != ";" {
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
