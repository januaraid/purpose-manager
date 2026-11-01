//! Workspaceを検証し、結果をJSONで出力する。
//!
//! 使い方: `cargo run -p purpose-core --example validate -- <Workspaceのフォルダ> [YYYY-MM-DD]`
//! エラーがあれば終了コード1を返す。

use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(root) = args.next().map(PathBuf::from) else {
        eprintln!("usage: validate <workspace-dir> [today]");
        std::process::exit(2);
    };
    let today = args.next();
    match purpose_core::load(&root, today.as_deref()) {
        Ok(snap) => {
            println!("{}", serde_json::to_string_pretty(&snap).unwrap());
            if snap.nodes.iter().any(|n| n.has_errors()) {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
}
