//! 目的YAMLの出力（仕様書 §4.6）。
//!
//! - キーは既定の順序で出力し、未知フィールドはその後に元の順序で出力する。
//! - UTF-8・LF・末尾改行。コメントは出力しない。
//! - 文字列はプレーン → ブロック → ダブルクォートの順に試し、読み戻して同じ値になる形を選ぶ。

use yaml_rust2::{Yaml, YamlLoader};

use crate::model::{Purpose, PurposeFields};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmitError {
    #[error("出力できない値が含まれています: {0}")]
    Unsupported(String),
}

/// 空の任意項目を省略するなど、保存前に値を正規化する。
pub fn normalize(mut f: PurposeFields) -> PurposeFields {
    let none_if_blank = |v: Option<String>| v.filter(|s| !s.trim().is_empty());
    f.rationale = none_if_blank(f.rationale);
    f.valid_from = none_if_blank(f.valid_from);
    f.valid_until = none_if_blank(f.valid_until);
    f.review_at = none_if_blank(f.review_at);
    f.parent = none_if_blank(f.parent);
    f.principles = f
        .principles
        .map(|ps| {
            ps.into_iter()
                .filter(|p| !p.trim().is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|ps| !ps.is_empty());
    f
}

pub fn emit_purpose(p: &Purpose) -> Result<String, EmitError> {
    let f = &p.fields;
    let s = |v: &str| Yaml::String(v.to_string());
    let mut entries: Vec<(Yaml, Yaml)> = vec![
        (s("schema_version"), Yaml::Integer(p.schema_version)),
        (s("id"), s(&f.id)),
        (
            s("parent"),
            f.parent.as_deref().map(s).unwrap_or(Yaml::Null),
        ),
        (s("name"), s(&f.name)),
        (s("purpose"), s(&f.purpose)),
    ];
    if let Some(r) = &f.rationale {
        entries.push((s("rationale"), s(r)));
    }
    if let Some(ps) = &f.principles {
        entries.push((
            s("principles"),
            Yaml::Array(ps.iter().map(|x| s(x)).collect()),
        ));
    }
    entries.push((s("status"), s(f.status.as_str())));
    for (key, value) in [
        ("valid_from", &f.valid_from),
        ("valid_until", &f.valid_until),
        ("review_at", &f.review_at),
    ] {
        if let Some(v) = value {
            entries.push((s(key), s(v)));
        }
    }
    entries.extend(p.extra.iter().cloned());
    emit_mapping(&entries)
}

/// トップレベルのマッピングを出力する。
pub fn emit_mapping(entries: &[(Yaml, Yaml)]) -> Result<String, EmitError> {
    let mut out = String::new();
    for (k, v) in entries {
        out.push_str(&emit_key(k)?);
        out.push(':');
        emit_node(v, 2, &mut out)?;
    }
    Ok(out)
}

/// `key:` や `-` の直後に続く部分（先頭の空白と末尾の改行を含む）を出力する。
fn emit_node(v: &Yaml, child_indent: usize, out: &mut String) -> Result<(), EmitError> {
    let pad = " ".repeat(child_indent);
    match v {
        Yaml::Array(items) if items.is_empty() => out.push_str(" []\n"),
        Yaml::Array(items) => {
            out.push('\n');
            for item in items {
                out.push_str(&pad);
                out.push('-');
                emit_node(item, child_indent + 2, out)?;
            }
        }
        Yaml::Hash(h) if h.is_empty() => out.push_str(" {}\n"),
        Yaml::Hash(h) => {
            out.push('\n');
            for (k, v) in h {
                out.push_str(&pad);
                out.push_str(&emit_key(k)?);
                out.push(':');
                emit_node(v, child_indent + 2, out)?;
            }
        }
        Yaml::String(s) => {
            if let Some(block) = literal_block(s, &pad) {
                out.push_str(&block);
            } else {
                out.push(' ');
                out.push_str(&inline_string(s));
                out.push('\n');
            }
        }
        other => {
            out.push(' ');
            out.push_str(&scalar(other)?);
            out.push('\n');
        }
    }
    Ok(())
}

fn emit_key(k: &Yaml) -> Result<String, EmitError> {
    match k {
        Yaml::String(s) => Ok(inline_string(s)),
        Yaml::Hash(_) | Yaml::Array(_) => Err(EmitError::Unsupported(
            "マッピングや配列をキーにした値".into(),
        )),
        other => scalar(other),
    }
}

fn scalar(v: &Yaml) -> Result<String, EmitError> {
    match v {
        Yaml::Null => Ok("null".into()),
        Yaml::Boolean(b) => Ok(b.to_string()),
        Yaml::Integer(i) => Ok(i.to_string()),
        Yaml::Real(r) => Ok(r.clone()),
        Yaml::String(s) => Ok(inline_string(s)),
        Yaml::Alias(_) => Err(EmitError::Unsupported("エイリアス".into())),
        Yaml::BadValue => Err(EmitError::Unsupported("不正な値".into())),
        Yaml::Array(_) | Yaml::Hash(_) => unreachable!("コレクションはemit_nodeで処理する"),
    }
}

/// `key: <value>` として読み戻したとき、同じ文字列になるか。
fn round_trips(rendered_value: &str, expected: &str) -> bool {
    let doc = format!("k:{rendered_value}");
    match YamlLoader::load_from_str(&doc) {
        Ok(docs) if docs.len() == 1 => docs[0]["k"].as_str() == Some(expected),
        _ => false,
    }
}

/// YAML 1.1 で真偽値として読まれうる語。他のツールでの誤読を避けるためクォートする。
const YAML11_BOOLISH: &[&str] = &["y", "n", "yes", "no", "on", "off"];

fn inline_string(s: &str) -> String {
    let plain_candidate = !s.is_empty()
        && s.trim() == s
        && !s.contains(['\n', '\r', '\t'])
        && !s.chars().any(char::is_control)
        && !YAML11_BOOLISH.contains(&s.to_ascii_lowercase().as_str());
    if plain_candidate && round_trips(&format!(" {s}\n"), s) {
        return s.to_string();
    }
    double_quoted(s)
}

fn double_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 複数行の文字列をリテラルブロック（`|`）で出力する。適さない場合は `None`。
fn literal_block(s: &str, pad: &str) -> Option<String> {
    if !s.contains('\n')
        || s.contains('\r')
        || s.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return None;
    }
    let content = s.trim_end_matches('\n');
    if content.is_empty() {
        return None;
    }
    let trailing = s.len() - content.len();
    let chomp = match trailing {
        0 => "-",
        1 => "",
        _ => "+",
    };
    let first_non_empty = content.lines().find(|l| !l.is_empty()).unwrap_or("");
    let indicator = if first_non_empty.starts_with([' ', '\t']) {
        "2"
    } else {
        ""
    };

    let mut out = format!(" |{indicator}{chomp}\n");
    for line in content.split('\n') {
        if !line.is_empty() {
            out.push_str(pad);
            out.push_str(line);
        }
        out.push('\n');
    }
    for _ in 1..trailing {
        out.push('\n');
    }

    // インデント幅が変わっても同じ値になることを確かめる（失敗したらダブルクォートへ）。
    let check: String = out
        .lines()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 || l.is_empty() {
                format!("{l}\n")
            } else {
                format!("  {}\n", &l[pad.len()..])
            }
        })
        .collect();
    round_trips(&check, s).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;
    use crate::parse::parse_purpose;

    fn fields() -> PurposeFields {
        PurposeFields {
            id: "acme-jobs".into(),
            parent: Some("acme-corp".into()),
            name: "Acme Jobs".into(),
            purpose: "求職者が自分に合った仕事に出会える状態をつくる。".into(),
            rationale: Some("一行目\n二行目\n".into()),
            principles: Some(vec![
                "求職者を第一に考える".into(),
                "シンプルさを保つ".into(),
            ]),
            status: Status::Active,
            valid_from: Some("2026-10-01".into()),
            valid_until: None,
            review_at: Some("2027-02-28".into()),
        }
    }

    fn round_trip(p: &Purpose) -> Purpose {
        let text = emit_purpose(p).unwrap();
        let parsed = parse_purpose(&text, &p.fields.id);
        assert!(
            parsed.diagnostics.is_empty(),
            "{text}\n{:?}",
            parsed.diagnostics
        );
        parsed.purpose.unwrap()
    }

    #[test]
    fn emits_in_canonical_order_with_plain_dates() {
        let text = emit_purpose(&Purpose::new(fields())).unwrap();
        assert_eq!(
            text,
            "schema_version: 1
id: acme-jobs
parent: acme-corp
name: Acme Jobs
purpose: 求職者が自分に合った仕事に出会える状態をつくる。
rationale: |
  一行目
  二行目
principles:
  - 求職者を第一に考える
  - シンプルさを保つ
status: active
valid_from: 2026-10-01
review_at: 2027-02-28
"
        );
    }

    #[test]
    fn root_parent_is_null() {
        let mut f = fields();
        f.parent = None;
        assert!(
            emit_purpose(&Purpose::new(f))
                .unwrap()
                .contains("parent: null\n")
        );
    }

    #[test]
    fn tricky_strings_round_trip() {
        let samples = [
            "true",
            "null",
            "~",
            "123",
            "1.5",
            "yes",
            "No",
            "on",
            "- dash",
            "key: value",
            "a #b",
            "#hash",
            "'quoted'",
            "\"dq\"",
            " leading",
            "trailing ",
            "",
            "[x]",
            "{y}",
            "*star",
            "&amp",
            "!tag",
            "%pct",
            "@at",
            "`bt",
            "a\tb",
            "line\r\nbreak",
            "\n\nonly-leading",
            "x\n\n\n",
            "  indented\nnext",
            "a\n  b\n",
            "end:",
            "\u{7}bell",
            "2026-10-01",
            "0x1f",
            ".inf",
            "日本語: コロン",
            "|",
            ">",
        ];
        for s in samples {
            let mut f = fields();
            f.purpose = s.to_string();
            f.rationale = Some(s.to_string());
            f.name = format!("n{s}");
            f.principles = Some(vec![s.to_string()]);
            if s.trim().is_empty() {
                continue;
            }
            let p = Purpose::new(f);
            assert_eq!(round_trip(&p), p, "sample {s:?}");
        }
    }

    #[test]
    fn preserves_unknown_fields_after_known_ones() {
        let text = "owner: someone
schema_version: 1
id: acme-jobs
tags:
  - a
  - nested:
      deep: [1, 2]
parent: acme-corp
name: Acme Jobs
purpose: P
status: active
meta: {}
score: 1.50
flag: false
";
        let p = parse_purpose(text, "acme-jobs").purpose.unwrap();
        let out = emit_purpose(&p).unwrap();
        assert!(
            out.starts_with("schema_version: 1\nid: acme-jobs\nparent: acme-corp\n"),
            "{out}"
        );
        assert!(out.ends_with("status: active\nowner: someone\ntags:\n  - a\n  -\n    nested:\n      deep:\n        - 1\n        - 2\nmeta: {}\nscore: 1.50\nflag: false\n"), "{out}");
        assert_eq!(round_trip(&p), p);
    }

    #[test]
    fn normalize_drops_blank_optionals() {
        let mut f = fields();
        f.rationale = Some("  ".into());
        f.principles = Some(vec!["".into()]);
        f.valid_from = Some("".into());
        let n = normalize(f);
        assert_eq!(n.rationale, None);
        assert_eq!(n.principles, None);
        assert_eq!(n.valid_from, None);
    }
}
