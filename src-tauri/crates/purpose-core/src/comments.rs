//! YAMLコメントの有無の検出（仕様書 §4.6：GUI保存時は保持しないため、保存前に警告する）。
//!
//! 安全側に倒した判定：コメントでない `#` をコメントと見なすこと（警告が余分に出る）はあっても、
//! 本当のコメントを見落とさないようにする。

pub fn has_comments(text: &str) -> bool {
    let mut quote: Option<char> = None;
    // ブロックスカラー（| や >）の親行のインデント。この値より深い行は本文。
    let mut block_parent_indent: Option<usize> = None;

    for line in text.lines() {
        let indent = line.len() - line.trim_start_matches(' ').len();
        let trimmed = line.trim();

        if let Some(parent) = block_parent_indent {
            if trimmed.is_empty() || indent > parent {
                continue;
            }
            block_parent_indent = None;
        }

        let chars: Vec<char> = line.chars().collect();
        let mut prev: Option<char> = None;
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            match quote {
                Some('"') => {
                    if c == '\\' {
                        i += 1; // エスケープされた次の文字を飛ばす
                    } else if c == '"' {
                        quote = None;
                    }
                }
                Some(_) => {
                    if c == '\'' {
                        if chars.get(i + 1) == Some(&'\'') {
                            i += 1; // '' はエスケープ
                        } else {
                            quote = None;
                        }
                    }
                }
                None => {
                    let at_token_start = matches!(
                        prev,
                        None | Some(' ' | '\t' | '[' | '{' | ',' | ':' | '-' | '?')
                    );
                    if c == '#' && matches!(prev, None | Some(' ' | '\t')) {
                        return true;
                    }
                    if (c == '"' || c == '\'') && at_token_start {
                        quote = Some(c);
                    }
                }
            }
            prev = Some(chars[i]);
            i += 1;
        }

        if quote.is_none() && opens_block_scalar(trimmed) {
            block_parent_indent = Some(indent);
        }
    }
    false
}

/// 行末が `|` や `>`（チョンピング・インデント指示子つきを含む）で終わるか。
fn opens_block_scalar(trimmed: &str) -> bool {
    let last = trimmed.rsplit([' ', '\t']).next().unwrap_or("");
    let before = &trimmed[..trimmed.len() - last.len()];
    let indicator_position = before.is_empty()
        || before.ends_with(": ")
        || before.trim_end().ends_with(':')
        || before.trim_end() == "-";
    let mut cs = last.chars();
    matches!(cs.next(), Some('|' | '>'))
        && cs.all(|c| matches!(c, '-' | '+' | '1'..='9'))
        && indicator_position
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_comments() {
        assert!(has_comments("# top\nid: a\n"));
        assert!(has_comments("id: a # trailing\n"));
        assert!(has_comments("list:\n  # inside\n  - a\n"));
        assert!(has_comments(
            "rationale: |\n  text\nname: x # after block\n"
        ));
        assert!(has_comments("a: 'x' # c\n"));
        assert!(has_comments("a: \"x\\\"\" # c\n"));
    }

    #[test]
    fn ignores_hashes_that_are_not_comments() {
        assert!(!has_comments("name: C#\n"));
        assert!(!has_comments("name: issue#12\n"));
        assert!(!has_comments("name: 'a # b'\n"));
        assert!(!has_comments("name: \"a # b\"\n"));
        assert!(!has_comments("name: 'it''s # fine'\n"));
        assert!(!has_comments(
            "rationale: |\n  # not a comment\n  text\nname: x\n"
        ));
        assert!(!has_comments("rationale: >-\n  line\n\n  # body\n"));
        assert!(!has_comments("items:\n  - |\n    # body\n"));
        assert!(!has_comments("name: it's ok\n"));
        assert!(!has_comments("q: \"multi\n  # still quoted\n  end\"\n"));
    }
}
