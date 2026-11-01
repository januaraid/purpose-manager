//! Workspaceの読み込みと、ツリー全体の検証（仕様書 §3.1、§4.4、§4.7）。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use yaml_rust2::Yaml;

use crate::comments::has_comments;
use crate::diagnostics::{Code, Diagnostic, Severity};
use crate::model::{PurposeFields, Status};
use crate::parse::{ParsedPurpose, decode, load_single_mapping, parse_bytes};
use crate::store::hash_bytes;

pub const PURPOSES_DIR: &str = "purposes";
pub const WORKSPACE_FILE: &str = "workspace.yaml";

/// 読み込んだ1ファイル分の情報。
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub file_name: String,
    pub stem: String,
    pub path: PathBuf,
    pub hash: String,
    pub has_comments: bool,
    pub parsed: ParsedPurpose,
}

impl FileEntry {
    pub fn read(path: &Path) -> std::io::Result<Self> {
        let bytes = fs::read(path)?;
        Ok(Self::from_bytes(path, &bytes))
    }

    pub fn from_bytes(path: &Path, bytes: &[u8]) -> Self {
        let file_name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stem = file_name
            .strip_suffix(".yaml")
            .unwrap_or(&file_name)
            .to_string();
        let parsed = parse_bytes(bytes, &stem);
        let comments = decode(bytes).is_some_and(|t| has_comments(&t));
        Self {
            file_name,
            stem,
            path: path.to_path_buf(),
            hash: hash_bytes(bytes),
            has_comments: comments,
            parsed,
        }
    }

    /// ファイルに書かれている親ID（ファイルが壊れていても読み取れた範囲で）。
    pub fn declared_parent(&self) -> Option<&str> {
        match &self.parsed.purpose {
            Some(p) => p.fields.parent.as_deref(),
            None => self.parsed.raw.parent.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Effective {
    /// 自身と全祖先が active。
    Active,
    /// 自身が archived。
    Archived,
    /// 自身は active だが、祖先のいずれかが archived。
    InheritedInactive,
    /// 祖先を辿れないため判定できない。
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    /// ファイル名から拡張子を除いたもの。Workspace内で一意で、ノードのキーに使う。
    pub id: String,
    pub file_name: String,
    pub path: String,
    pub hash: String,
    pub has_comments: bool,
    /// ファイル単位のエラーがなければ `Some`。`None` のノードはGUIで編集できない。
    pub fields: Option<PurposeFields>,
    pub raw_name: Option<String>,
    pub raw_parent: Option<String>,
    pub unknown_fields: Vec<String>,
    /// ルートまで祖先を辿れるか。
    pub resolved: bool,
    /// ルートから親までのID（`resolved` のときのみ）。
    pub ancestors: Vec<String>,
    pub children: Vec<String>,
    pub effective: Effective,
    /// `InheritedInactive` のとき、原因となったアーカイブ済み祖先のID。
    pub inactive_because: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Node {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub root: String,
    pub name: String,
    pub nodes: Vec<Node>,
    /// Workspace全体の診断。
    pub diagnostics: Vec<Diagnostic>,
}

impl Snapshot {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("フォルダが見つかりません: {0}")]
    NotFound(String),
    #[error("Workspaceではありません（purposes フォルダがありません）: {0}")]
    NotAWorkspace(String),
    #[error("読み込みに失敗しました: {0}")]
    Io(#[from] std::io::Error),
}

pub fn purposes_dir(root: &Path) -> PathBuf {
    root.join(PURPOSES_DIR)
}

pub fn purpose_path(root: &Path, id: &str) -> PathBuf {
    purposes_dir(root).join(format!("{id}.yaml"))
}

/// `purposes/` 直下の `.yaml` を読み込む。`.yml` は読み込まず警告する。
pub fn read_entries(root: &Path) -> Result<(Vec<FileEntry>, Vec<Diagnostic>), LoadError> {
    if !root.is_dir() {
        return Err(LoadError::NotFound(root.display().to_string()));
    }
    let dir = purposes_dir(root);
    if !dir.is_dir() {
        return Err(LoadError::NotAWorkspace(root.display().to_string()));
    }
    let mut entries = Vec::new();
    let mut diags = Vec::new();
    for item in fs::read_dir(&dir)? {
        let item = item?;
        if !item.file_type()?.is_file() {
            continue;
        }
        let name = item.file_name().to_string_lossy().into_owned();
        if name.ends_with(".yaml") {
            entries.push(FileEntry::read(&item.path())?);
        } else if name.ends_with(".yml") || name.to_ascii_lowercase().ends_with(".yaml") {
            diags.push(
                Diagnostic::warning(
                    Code::InvalidFileName,
                    format!("「{name}」は読み込みません。拡張子は小文字の .yaml にしてください"),
                )
                .file(&name),
            );
        }
    }
    entries.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok((entries, diags))
}

/// `workspace.yaml` から表示名を読む。なければフォルダ名を使う。
pub fn read_workspace_name(root: &Path) -> (String, Vec<Diagnostic>) {
    let fallback = root
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string());
    let path = root.join(WORKSPACE_FILE);
    let Ok(bytes) = fs::read(&path) else {
        return (
            fallback,
            vec![Diagnostic::warning(
                Code::WorkspaceFileMissing,
                "workspace.yaml がないため、フォルダ名を表示名に使います",
            )],
        );
    };
    let invalid = |msg: &str| {
        Diagnostic::warning(
            Code::WorkspaceFileInvalid,
            format!("workspace.yaml: {msg}。フォルダ名を表示名に使います"),
        )
    };
    let Some(text) = decode(&bytes) else {
        return (fallback, vec![invalid("UTF-8として読み込めません")]);
    };
    match load_single_mapping(&text) {
        Ok(map) => match map.get(&Yaml::String("name".into())) {
            Some(Yaml::String(n)) if !n.trim().is_empty() => (n.clone(), vec![]),
            _ => (
                fallback,
                vec![invalid("name がないか、文字列ではありません")],
            ),
        },
        Err(d) => (fallback, vec![invalid(&d.message)]),
    }
}

pub fn load(root: &Path, today: Option<&str>) -> Result<Snapshot, LoadError> {
    let (entries, mut diags) = read_entries(root)?;
    let (name, name_diags) = read_workspace_name(root);
    diags.extend(name_diags);
    let (nodes, ws_diags) = analyze(&entries, today);
    diags.extend(ws_diags);
    Ok(Snapshot {
        root: root.display().to_string(),
        name,
        nodes,
        diagnostics: diags,
    })
}

/// ファイル群からツリーを組み立て、Workspace全体の検証を行う。
/// 戻り値は（ノード一覧, Workspace全体の診断）。
pub fn analyze(entries: &[FileEntry], today: Option<&str>) -> (Vec<Node>, Vec<Diagnostic>) {
    let by_stem: HashMap<&str, &FileEntry> = entries.iter().map(|e| (e.stem.as_str(), e)).collect();
    let ok: HashMap<&str, &PurposeFields> = entries
        .iter()
        .filter_map(|e| {
            e.parsed
                .purpose
                .as_ref()
                .map(|p| (e.stem.as_str(), &p.fields))
        })
        .collect();
    let mut extra_diags: HashMap<&str, Vec<Diagnostic>> = HashMap::new();
    let mut ws_diags = Vec::new();

    // 重複ID：ファイルに書かれたIDが複数ファイルで同じ
    let mut by_declared_id: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in entries {
        if let Some(id) = e.parsed.raw.id.as_deref() {
            by_declared_id
                .entry(id)
                .or_default()
                .push(e.file_name.as_str());
        }
    }
    for e in entries {
        if let Some(files) = e
            .parsed
            .raw
            .id
            .as_deref()
            .and_then(|id| by_declared_id.get(id))
            .filter(|f| f.len() > 1)
        {
            extra_diags.entry(e.stem.as_str()).or_default().push(
                Diagnostic::error(
                    Code::DuplicateId,
                    format!(
                        "ID「{}」が複数のファイルにあります: {}",
                        e.parsed.raw.id.as_deref().unwrap(),
                        files.join(", ")
                    ),
                )
                .field("id"),
            );
        }
    }

    // 祖先を辿る。戻り値：(親から順の祖先, 終端の状態)
    enum Chain {
        Root,
        Broken,
        Cycle(Vec<String>),
    }
    let walk = |start: &str| -> (Vec<String>, Chain) {
        let mut seen = vec![start.to_string()];
        let mut cur = start;
        loop {
            let Some(parent) = ok.get(cur).and_then(|f| f.parent.as_deref()) else {
                return (seen[1..].to_vec(), Chain::Root);
            };
            if let Some(pos) = seen.iter().position(|s| s == parent) {
                return (seen[1..].to_vec(), Chain::Cycle(seen[pos..].to_vec()));
            }
            if !ok.contains_key(parent) {
                return (seen[1..].to_vec(), Chain::Broken);
            }
            seen.push(parent.to_string());
            cur = ok.get_key_value(parent).unwrap().0;
        }
    };

    let mut chains: HashMap<&str, (Vec<String>, bool)> = HashMap::new();
    let mut in_cycle: HashSet<String> = HashSet::new();
    for &id in ok.keys() {
        let (ancestors, end) = walk(id);
        match end {
            Chain::Root => {
                chains.insert(id, (ancestors, true));
            }
            Chain::Broken => {
                chains.insert(id, (ancestors, false));
            }
            Chain::Cycle(members) => {
                in_cycle.extend(members);
                chains.insert(id, (ancestors, false));
            }
        }
    }

    for &id in ok.keys() {
        let fields = ok[id];
        let d = extra_diags.entry(id).or_default();
        let resolved = chains[id].1;
        if in_cycle.contains(id) {
            d.push(Diagnostic::error(Code::Cycle, "親子関係が循環しています").field("parent"));
        } else if let Some(parent) = fields.parent.as_deref() {
            if !by_stem.contains_key(parent) {
                d.push(Diagnostic::error(Code::ParentNotFound, format!("親目的「{parent}」が見つかりません（purposes/{parent}.yaml がありません）")).field("parent"));
            } else if !resolved {
                d.push(
                    Diagnostic::warning(
                        Code::ParentUnresolved,
                        "祖先のファイルにエラーがあるため、ルートまで辿れません",
                    )
                    .field("parent"),
                );
            }
        }
    }

    // ルートの数
    let roots: Vec<&str> = {
        let mut r: Vec<&str> = ok
            .iter()
            .filter(|(_, f)| f.parent.is_none())
            .map(|(id, _)| *id)
            .collect();
        r.sort();
        r
    };
    if roots.len() > 1 {
        ws_diags.push(Diagnostic::warning(
            Code::MultipleRoots,
            format!(
                "ルートが複数あります: {}。ルートは原則1つです",
                roots.join(", ")
            ),
        ));
    }

    let mut children: HashMap<&str, Vec<String>> = HashMap::new();
    for (&id, f) in &ok {
        if let Some(p) = f.parent.as_deref().filter(|p| ok.contains_key(p)) {
            children
                .entry(ok.get_key_value(p).unwrap().0)
                .or_default()
                .push(id.to_string());
        }
    }
    for c in children.values_mut() {
        c.sort();
    }

    let mut nodes = Vec::with_capacity(entries.len());
    for e in entries {
        let id = e.stem.as_str();
        let mut diagnostics: Vec<Diagnostic> = e.parsed.diagnostics.clone();
        diagnostics.extend(extra_diags.remove(id).unwrap_or_default());

        let fields = e.parsed.purpose.as_ref().map(|p| p.fields.clone());
        let (parent_first, resolved) = chains.get(id).cloned().unwrap_or((vec![], false));
        let ancestors: Vec<String> = if resolved {
            parent_first.iter().rev().cloned().collect()
        } else {
            vec![]
        };

        let (effective, inactive_because) = match &fields {
            None => (Effective::Unknown, None),
            Some(f) if f.status == Status::Archived => (Effective::Archived, None),
            Some(_) => match parent_first.iter().find(|a| {
                ok.get(a.as_str())
                    .is_some_and(|f| f.status == Status::Archived)
            }) {
                Some(a) => (Effective::InheritedInactive, Some(a.clone())),
                None if resolved => (Effective::Active, None),
                None => (Effective::Unknown, None),
            },
        };
        if let Some(a) = &inactive_because {
            diagnostics.push(Diagnostic::info(
                Code::InheritedInactive,
                format!("祖先「{a}」がアーカイブされているため、実効的に非アクティブです"),
            ));
        }

        if let Some(f) = &fields {
            if resolved {
                period_warnings(f, &parent_first, &ok, &mut diagnostics);
            }
            if let (Some(review), Some(today)) = (&f.review_at, today)
                && f.status == Status::Active
                && review.as_str() < today
            {
                diagnostics.push(
                    Diagnostic::warning(
                        Code::ReviewOverdue,
                        format!("見直し予定日（{review}）を過ぎています"),
                    )
                    .field("review_at"),
                );
            }
        }

        let unknown_fields = e
            .parsed
            .purpose
            .as_ref()
            .map(|p| p.unknown_keys())
            .unwrap_or_default();
        if !unknown_fields.is_empty() {
            diagnostics.push(Diagnostic::info(
                Code::UnknownFields,
                format!(
                    "未知のフィールドがあります（保存しても保持されます）: {}",
                    unknown_fields.join(", ")
                ),
            ));
        }
        if e.has_comments {
            diagnostics.push(Diagnostic::info(
                Code::HasComments,
                "YAMLコメントがあります。GUIで保存するとコメントは失われます",
            ));
        }
        for d in &mut diagnostics {
            d.file = Some(e.file_name.clone());
        }

        nodes.push(Node {
            id: id.to_string(),
            file_name: e.file_name.clone(),
            path: e.path.display().to_string(),
            hash: e.hash.clone(),
            has_comments: e.has_comments,
            fields,
            raw_name: e.parsed.raw.name.clone(),
            raw_parent: e.declared_parent().map(str::to_string),
            unknown_fields,
            resolved,
            ancestors,
            children: children.remove(id).unwrap_or_default(),
            effective,
            inactive_because,
            diagnostics,
        });
    }
    (nodes, ws_diags)
}

/// 子の有効期間が、もっとも近い祖先の有効期間からはみ出していないか。
fn period_warnings(
    f: &PurposeFields,
    parent_first: &[String],
    ok: &HashMap<&str, &PurposeFields>,
    diags: &mut Vec<Diagnostic>,
) {
    let nearest = |pick: fn(&PurposeFields) -> Option<&String>| {
        parent_first.iter().find_map(|a| {
            ok.get(a.as_str())
                .and_then(|af| pick(af).map(|v| (a.clone(), v.clone())))
        })
    };
    if let (Some(from), Some((a, af))) = (&f.valid_from, nearest(|x| x.valid_from.as_ref()))
        && from < &af
    {
        diags.push(
            Diagnostic::warning(
                Code::PeriodOutsideParent,
                format!("valid_from（{from}）が祖先「{a}」の開始日（{af}）より前です"),
            )
            .field("valid_from"),
        );
    }
    if let (Some(until), Some((a, au))) = (&f.valid_until, nearest(|x| x.valid_until.as_ref()))
        && until > &au
    {
        diags.push(
            Diagnostic::warning(
                Code::PeriodOutsideParent,
                format!("valid_until（{until}）が祖先「{a}」の終了日（{au}）より後です"),
            )
            .field("valid_until"),
        );
    }
}
