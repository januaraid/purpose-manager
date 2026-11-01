//! `CLAUDE.md` 向け参照指示の生成（仕様書 §5.1）。`CLAUDE.md` 自体は編集しない。

use std::path::Path;

use crate::workspace::{PURPOSES_DIR, purpose_path};

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
