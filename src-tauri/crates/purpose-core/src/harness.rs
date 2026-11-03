//! `CLAUDE.md` 向け参照指示の生成（仕様書 §5.1）。`CLAUDE.md` 自体は編集しない。

use std::path::Path;

use crate::id::validate_id;
use crate::workspace::{PURPOSES_DIR, purpose_path};

/// 指定した目的のYAMLを、ファイルの内容のまま1つのテキストにつなげる（AIへの相談用）。
///
/// 各ファイルを YAML の複数文書（`---` 区切り）として並べ、先頭にファイル名のコメントを付ける。
/// 改行は LF にそろえ、BOM は除く。それ以外（コメントを含む）は変更しない。
pub fn concat_yaml(root: &Path, ids: &[String], title: &str) -> Result<String, String> {
    let mut docs = Vec::with_capacity(ids.len());
    for id in ids {
        validate_id(id).map_err(|e| format!("{id}: {e}"))?;
        let path = purpose_path(root, id);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let text = String::from_utf8_lossy(&bytes);
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text).replace("\r\n", "\n");
        let body = text.trim_end_matches('\n');
        docs.push(format!("---\n# {PURPOSES_DIR}/{id}.yaml\n{body}\n"));
    }
    let mut out = String::new();
    for line in title.lines() {
        out.push_str("# ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&format!(
        "# 目的 {} 件。親から子の順に並べています。各文書の parent は親の id を指します。\n",
        docs.len()
    ));
    for d in docs {
        out.push_str(&d);
    }
    Ok(out)
}

pub fn claude_md_snippet(root: &Path, id: &str) -> String {
    let file = purpose_path(root, id);
    let pattern = root.join(PURPOSES_DIR).join("<ID>.yaml");
    format!(
        "## Purpose

現在の作業目的:
`{file}`

親目的は`parent`のIDに対応する
`{pattern}`にあります。

親をルートまで辿り、各階層の目的と判断原則を参照してください。

目的の変更には人間の承認が必要です。
目標設定と実行方法は自由に決定してください。
",
        file = file.display(),
        pattern = pattern.display(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concatenates_files_as_is() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("purposes");
        std::fs::create_dir(&p).unwrap();
        std::fs::write(p.join("a.yaml"), "\u{feff}id: a\r\n# コメント\r\nname: A\r\n\r\n").unwrap();
        std::fs::write(p.join("b.yaml"), "id: b\nparent: a\n").unwrap();
        let s = concat_yaml(dir.path(), &["a".into(), "b".into()], "WS").unwrap();
        assert_eq!(
            s,
            "# WS\n# 目的 2 件。親から子の順に並べています。各文書の parent は親の id を指します。\n\
             ---\n# purposes/a.yaml\nid: a\n# コメント\nname: A\n\
             ---\n# purposes/b.yaml\nid: b\nparent: a\n"
        );
    }

    #[test]
    fn rejects_paths_outside_purposes() {
        let dir = tempfile::tempdir().unwrap();
        assert!(concat_yaml(dir.path(), &["../secret".into()], "").is_err());
        assert!(concat_yaml(dir.path(), &["missing".into()], "").is_err());
    }

    #[test]
    fn snippet_contains_paths_and_contract() {
        let root = Path::new("ws");
        let s = claude_md_snippet(root, "job-matching");
        assert!(
            s.contains(
                &Path::new("ws")
                    .join("purposes")
                    .join("job-matching.yaml")
                    .display()
                    .to_string()
            )
        );
        assert!(
            s.contains(
                &Path::new("ws")
                    .join("purposes")
                    .join("<ID>.yaml")
                    .display()
                    .to_string()
            )
        );
        assert!(s.contains("人間の承認"));
    }
}
