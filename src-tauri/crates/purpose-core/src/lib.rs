//! Purpose Manager のドメイン処理。GUIやTauriに依存しない。
//!
//! 機能仕様の正本は `docs/specification.md`。

pub mod comments;
pub mod date;
pub mod diagnostics;
pub mod emit;
pub mod harness;
pub mod id;
pub mod model;
pub mod parse;
pub mod store;
pub mod workspace;

pub use diagnostics::{Code, Diagnostic, Severity};
pub use model::{Purpose, PurposeFields, Status};
pub use store::{
    SaveError, SaveOutcome, SaveRequest, delete_purpose, init_workspace, save_purpose,
};
pub use workspace::{Effective, LoadError, Node, Snapshot, load};
