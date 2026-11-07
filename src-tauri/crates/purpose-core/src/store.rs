//! 安全な保存・削除・Workspace初期化（仕様書 §4.6、計画書 Phase 1）。
//!
//! - 読み込み時のハッシュと保存直前のハッシュを比べ、外部の変更を黙って上書きしない。
//! - 一時ファイルに書いてから置き換え、失敗しても元ファイルを壊さない。
//! - 書き出す内容を読み戻して検証してから保存する。

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use yaml_rust2::Yaml;

use crate::diagnostics::{Diagnostic, Severity};
use crate::emit::{emit_mapping, emit_purpose, normalize};
use crate::id::validate_id;
use crate::model::{Purpose, PurposeFields};
use crate::parse::parse_purpose;
use crate::workspace::{
    FileEntry, PURPOSES_DIR, WORKSPACE_FILE, analyze, purpose_path, read_entries,
};

pub fn hash_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveRequest {
    pub fields: PurposeFields,
    /// 編集を始めたときのファイルのハッシュ。`None` は新規作成。
    pub expected_hash: Option<String>,
    /// コメントを含むファイルの保存で、コメントが失われることを利用者が了承したか。
    #[serde(default)]
    pub allow_comment_loss: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaveOutcome {
    pub id: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SaveError {
    #[error("入力に問題があります")]
    Validation { diagnostics: Vec<Diagnostic> },
    #[error("ファイルが外部で変更されています。再読み込みしてください")]
    Conflict { current_hash: Option<String> },
    #[error("このファイルにはYAMLコメントがあります。保存するとコメントは失われます")]
    CommentsWouldBeLost,
    #[error("同じIDの目的がすでにあります")]
    AlreadyExists,
    #[error("ファイルにエラーがあるため、GUIでは編集できません。外部エディタで修正してください")]
    NotEditable { diagnostics: Vec<Diagnostic> },
    #[error("子目的があるため削除できません: {children:?}")]
    HasChildren { children: Vec<String> },
    #[error("Workspaceを読み込めません: {message}")]
    Workspace { message: String },
    #[error("ファイル操作に失敗しました: {message}")]
    Io { message: String },
}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        SaveError::Io {
            message: e.to_string(),
        }
    }
}

fn current_hash(path: &Path) -> Result<Option<String>, SaveError> {
    match fs::read(path) {
        Ok(b) => Ok(Some(hash_bytes(&b))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// 同じフォルダの一時ファイルに書き、同期してから置き換える。
pub fn atomic_write(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("保存先のフォルダがありません"))?;
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(content)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

pub fn save_purpose(root: &Path, req: SaveRequest) -> Result<SaveOutcome, SaveError> {
    let fields = normalize(req.fields);
    if let Err(msg) = validate_id(&fields.id) {
        return Err(SaveError::Validation {
            diagnostics: vec![
                Diagnostic::error(crate::diagnostics::Code::InvalidId, msg).field("id"),
            ],
        });
    }
    let id = fields.id.clone();
    let path = purpose_path(root, &id);

    // 1. 現在のファイルを確認する
    let extra: Vec<(Yaml, Yaml)> = match &req.expected_hash {
        None => {
            if path.exists() {
                return Err(SaveError::AlreadyExists);
            }
            vec![]
        }
        Some(expected) => {
            let bytes = match fs::read(&path) {
                Ok(b) => b,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Err(SaveError::Conflict { current_hash: None });
                }
                Err(e) => return Err(e.into()),
            };
            let current = FileEntry::from_bytes(&path, &bytes);
            if &current.hash != expected {
                return Err(SaveError::Conflict {
                    current_hash: Some(current.hash),
                });
            }
            let Some(existing) = current.parsed.purpose else {
                return Err(SaveError::NotEditable {
                    diagnostics: current.parsed.diagnostics,
                });
            };
            if current.has_comments && !req.allow_comment_loss {
                return Err(SaveError::CommentsWouldBeLost);
            }
            existing.extra
        }
    };

    // 2. 書き出す内容を作り、読み戻して検証する
    let candidate = Purpose {
        schema_version: crate::model::SCHEMA_VERSION,
        fields,
        extra,
    };
    let text = emit_purpose(&candidate).map_err(|e| SaveError::Io {
        message: e.to_string(),
    })?;
    let reparsed = parse_purpose(&text, &id);
    if crate::diagnostics::has_errors(&reparsed.diagnostics) {
        return Err(SaveError::Validation {
            diagnostics: reparsed.diagnostics,
        });
    }
    if reparsed.purpose.as_ref() != Some(&candidate) {
        return Err(SaveError::Io {
            message: "書き出す内容を読み戻したときに値が一致しません（保存を中止しました）".into(),
        });
    }

    // 3. 保存後のWorkspaceで、このノードにエラーが出ないか確認する
    check_in_workspace(root, &id, &text)?;

    // 4. 書き込む直前にもう一度、外部の変更がないか確認する
    let now = current_hash(&path)?;
    if now != req.expected_hash {
        return Err(SaveError::Conflict { current_hash: now });
    }
    atomic_write(&path, text.as_bytes())?;
    Ok(SaveOutcome {
        id,
        hash: hash_bytes(text.as_bytes()),
    })
}

/// 保存後のWorkspaceを想定して検証し、このノードにエラーが出るなら保存させない。
fn check_in_workspace(root: &Path, id: &str, text: &str) -> Result<(), SaveError> {
    let path = purpose_path(root, id);
    let (mut entries, _) = read_entries(root).map_err(|e| SaveError::Workspace {
        message: e.to_string(),
    })?;
    let new_entry = FileEntry::from_bytes(&path, text.as_bytes());
    match entries.iter_mut().find(|e| e.stem == id) {
        Some(e) => *e = new_entry,
        None => entries.push(new_entry),
    }
    let (nodes, _) = analyze(&entries, None);
    let errors: Vec<Diagnostic> = nodes
        .iter()
        .find(|n| n.id == id)
        .map(|n| {
            n.diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(SaveError::Validation {
            diagnostics: errors,
        })
    }
}

/// YAMLを直接編集するために読み込んだファイルの内容。
#[derive(Debug, Clone, Serialize)]
pub struct PurposeText {
    pub text: String,
    pub hash: String,
}

/// ファイル名として安全なID（ノードのキー）か。壊れたファイル名のノードも読めるよう、IDの規則より緩くする。
fn safe_stem(id: &str) -> Result<(), SaveError> {
    if id.is_empty() || id.contains(['/', '\\']) || id.contains("..") {
        return Err(SaveError::Io {
            message: format!("ファイル名として使えないIDです: {id}"),
        });
    }
    Ok(())
}

/// 目的ファイルの内容をそのまま読み込む（BOMは除き、改行はLFにそろえる）。
pub fn read_purpose_text(root: &Path, id: &str) -> Result<PurposeText, SaveError> {
    safe_stem(id)?;
    let bytes = fs::read(purpose_path(root, id))?;
    let text = crate::parse::decode(&bytes).ok_or_else(|| SaveError::Io {
        message: "UTF-8として読み込めないため、YAMLを直接編集できません".into(),
    })?;
    Ok(PurposeText {
        text,
        hash: hash_bytes(&bytes),
    })
}

/// 利用者が直接編集したYAMLを、そのまま（コメントや書式も含めて）保存する。
/// フォームからの保存と同じく、検証・競合検知・安全な置き換えをする。
pub fn save_purpose_text(
    root: &Path,
    id: &str,
    text: &str,
    expected_hash: &str,
) -> Result<SaveOutcome, SaveError> {
    safe_stem(id)?;
    write_purpose_text(root, id, text, Some(expected_hash))
}

/// 利用者が書いたYAMLから新しい目的を作る。ファイル名は YAML の `id` から決める。
pub fn create_purpose_text(root: &Path, text: &str) -> Result<SaveOutcome, SaveError> {
    let validation = |d: Diagnostic| SaveError::Validation {
        diagnostics: vec![d],
    };
    let map = crate::parse::load_single_mapping(&text.replace("\r\n", "\n")).map_err(validation)?;
    let id = match map.get(&Yaml::String("id".into())) {
        Some(Yaml::String(id)) => id.clone(),
        _ => {
            return Err(validation(
                Diagnostic::error(
                    crate::diagnostics::Code::MissingField,
                    "id がありません。ファイル名になるIDを文字列で書いてください",
                )
                .field("id"),
            ));
        }
    };
    if let Err(msg) = validate_id(&id) {
        return Err(validation(
            Diagnostic::error(crate::diagnostics::Code::InvalidId, msg).field("id"),
        ));
    }
    if purpose_path(root, &id).exists() {
        return Err(SaveError::AlreadyExists);
    }
    write_purpose_text(root, &id, text, None)
}

/// 新しい目的のYAMLのひな形（フォームに入力済みの内容を入れる）。
pub fn purpose_text_template(fields: PurposeFields) -> Result<String, SaveError> {
    emit_purpose(&Purpose::new(normalize(fields))).map_err(|e| SaveError::Io {
        message: e.to_string(),
    })
}

/// YAMLの文字列を検証して書き込む。`expected_hash` が `None` なら新規作成。
fn write_purpose_text(
    root: &Path,
    id: &str,
    text: &str,
    expected_hash: Option<&str>,
) -> Result<SaveOutcome, SaveError> {
    let path = purpose_path(root, id);
    let mut text = text.replace("\r\n", "\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }

    // 1. ファイル単位の検証（構文・必須項目・IDとファイル名の一致など）
    let parsed = parse_purpose(&text, id);
    if crate::diagnostics::has_errors(&parsed.diagnostics) {
        return Err(SaveError::Validation {
            diagnostics: parsed
                .diagnostics
                .into_iter()
                .filter(|d| d.severity == Severity::Error)
                .collect(),
        });
    }

    // 2. 保存後のWorkspaceで、このノードにエラーが出ないか確認する
    check_in_workspace(root, id, &text)?;

    // 3. 書き込む直前に、外部の変更がないか（新規なら同じIDのファイルができていないか）確認する
    let now = current_hash(&path)?;
    if expected_hash.is_none() && now.is_some() {
        return Err(SaveError::AlreadyExists);
    }
    if now.as_deref() != expected_hash {
        return Err(SaveError::Conflict { current_hash: now });
    }
    atomic_write(&path, text.as_bytes())?;
    Ok(SaveOutcome {
        id: id.to_string(),
        hash: hash_bytes(text.as_bytes()),
    })
}

/// 子を持たない目的だけを物理削除する（仕様書 §10）。
pub fn delete_purpose(root: &Path, id: &str, expected_hash: &str) -> Result<(), SaveError> {
    let path = purpose_path(root, id);
    let (entries, _) = read_entries(root).map_err(|e| SaveError::Workspace {
        message: e.to_string(),
    })?;
    let mut children: Vec<String> = entries
        .iter()
        .filter(|e| e.stem != id && e.declared_parent() == Some(id))
        .map(|e| e.stem.clone())
        .collect();
    if !children.is_empty() {
        children.sort();
        return Err(SaveError::HasChildren { children });
    }
    let now = current_hash(&path)?;
    if now.as_deref() != Some(expected_hash) {
        return Err(SaveError::Conflict { current_hash: now });
    }
    fs::remove_file(&path)?;
    Ok(())
}

/// `workspace.yaml` と `purposes/` を作る。既存の `workspace.yaml` は上書きしない。
pub fn init_workspace(root: &Path, name: &str) -> Result<(), SaveError> {
    fs::create_dir_all(root.join(PURPOSES_DIR))?;
    let ws = root.join(WORKSPACE_FILE);
    if !ws.exists() {
        let text = emit_mapping(&[(Yaml::String("name".into()), Yaml::String(name.to_string()))])
            .map_err(|e| SaveError::Io {
            message: e.to_string(),
        })?;
        atomic_write(&ws, text.as_bytes())?;
    }
    Ok(())
}
