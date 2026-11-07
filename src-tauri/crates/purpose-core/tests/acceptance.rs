//! 仕様書 §9 の受け入れ条件のうち、データ・継承・時間・保存に関するもの（計画書 Phase 1 の完了条件）。

use std::fs;
use std::path::{Path, PathBuf};

use purpose_core::workspace::purpose_path;
use purpose_core::*;

const TODAY: &str = "2026-11-02";

fn sample_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/sample-workspace")
}

/// サンプルWorkspaceを一時フォルダへ複製する。
fn sample_copy() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("purposes")).unwrap();
    fs::copy(
        sample_dir().join("workspace.yaml"),
        dir.path().join("workspace.yaml"),
    )
    .unwrap();
    for e in fs::read_dir(sample_dir().join("purposes")).unwrap() {
        let e = e.unwrap();
        fs::copy(e.path(), dir.path().join("purposes").join(e.file_name())).unwrap();
    }
    dir
}

fn write(root: &Path, id: &str, body: &str) {
    fs::create_dir_all(root.join("purposes")).unwrap();
    fs::write(purpose_path(root, id), body).unwrap();
}

fn purpose_yaml(id: &str, parent: Option<&str>, status: &str) -> String {
    format!(
        "schema_version: 1\nid: {id}\nparent: {}\nname: {id}\npurpose: {id} の目的\nstatus: {status}\n",
        parent.unwrap_or("null")
    )
}

fn codes(node: &Node) -> Vec<Code> {
    node.diagnostics.iter().map(|d| d.code).collect()
}

fn fields_of(snap: &Snapshot, id: &str) -> PurposeFields {
    snap.node(id).unwrap().fields.clone().unwrap()
}

fn save(root: &Path, snap: &Snapshot, fields: PurposeFields) -> Result<SaveOutcome, SaveError> {
    let hash = snap.node(&fields.id).map(|n| n.hash.clone());
    save_purpose(
        root,
        SaveRequest {
            fields,
            expected_hash: hash,
            allow_comment_loss: false,
        },
    )
}

#[test]
fn sample_workspace_is_clean() {
    let snap = load(&sample_dir(), Some(TODAY)).unwrap();
    assert_eq!(snap.name, "Acme Corp（サンプル）"); // 受け入れ条件19
    assert!(snap.diagnostics.is_empty(), "{:?}", snap.diagnostics);
    for n in &snap.nodes {
        assert!(
            n.diagnostics.iter().all(|d| d.severity == Severity::Info),
            "{}: {:?}",
            n.id,
            n.diagnostics
        );
    }
}

#[test]
fn five_levels_and_ancestors() {
    // 受け入れ条件1・2・17
    let snap = load(&sample_dir(), Some(TODAY)).unwrap();
    let leaf = snap.node("search-relevance-2026h2").unwrap();
    assert!(leaf.resolved);
    assert_eq!(
        leaf.ancestors,
        [
            "acme-corp",
            "acme-jobs",
            "job-matching",
            "search-experience"
        ]
    );
    assert_eq!(leaf.effective, Effective::Active);
    let root = snap.node("acme-corp").unwrap();
    assert_eq!(root.children, ["acme-jobs", "legacy-portal"]);
    assert!(fields_of(&snap, "acme-corp").principles.unwrap().len() >= 2);
}

#[test]
fn archived_parent_makes_children_inactive_without_editing_files() {
    // 受け入れ条件12
    let dir = sample_copy();
    let before = fs::read(purpose_path(dir.path(), "legacy-data-migration")).unwrap();
    let snap = load(dir.path(), Some(TODAY)).unwrap();
    let child = snap.node("legacy-data-migration").unwrap();
    assert_eq!(child.fields.as_ref().unwrap().status, Status::Active);
    assert_eq!(child.effective, Effective::InheritedInactive);
    assert_eq!(child.inactive_because.as_deref(), Some("legacy-portal"));
    assert_eq!(
        fs::read(purpose_path(dir.path(), "legacy-data-migration")).unwrap(),
        before
    );
}

#[test]
fn archived_purposes_stay_addressable() {
    // 受け入れ条件11・13
    let snap = load(&sample_dir(), Some(TODAY)).unwrap();
    let old = snap.node("search-relevance-2026h1").unwrap();
    assert_eq!(old.effective, Effective::Archived);
    assert!(snap.node("search-relevance-2026h2").is_some());
}

#[test]
fn dates_never_change_status() {
    // 受け入れ条件9・10：期限を過ぎても status は変わらない
    let snap = load(&sample_dir(), Some("2030-01-01")).unwrap();
    let n = snap.node("search-relevance-2026h2").unwrap();
    assert_eq!(n.effective, Effective::Active);
    assert!(codes(n).contains(&Code::ReviewOverdue));
    assert_eq!(snap.node("acme-corp").unwrap().effective, Effective::Active);
}

#[test]
fn period_outside_parent_is_warning() {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    write(
        r,
        "root",
        &format!(
            "{}valid_from: 2026-04-01\nvalid_until: 2027-03-31\n",
            purpose_yaml("root", None, "active")
        ),
    );
    write(
        r,
        "child",
        &format!(
            "{}valid_from: 2026-01-01\nvalid_until: 2027-06-30\n",
            purpose_yaml("child", Some("root"), "active")
        ),
    );
    let snap = load(r, None).unwrap();
    let c = codes(snap.node("child").unwrap());
    assert_eq!(
        c.iter()
            .filter(|c| **c == Code::PeriodOutsideParent)
            .count(),
        2
    );
    assert!(
        snap.node("child")
            .unwrap()
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error)
    );
}

#[test]
fn detects_structural_errors() {
    // 受け入れ条件7
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    write(r, "root", &purpose_yaml("root", None, "active"));
    write(
        r,
        "orphan",
        &purpose_yaml("orphan", Some("missing"), "active"),
    );
    write(
        r,
        "loop-a",
        &purpose_yaml("loop-a", Some("loop-b"), "active"),
    );
    write(
        r,
        "loop-b",
        &purpose_yaml("loop-b", Some("loop-a"), "active"),
    );
    write(
        r,
        "below-loop",
        &purpose_yaml("below-loop", Some("loop-a"), "active"),
    );
    write(r, "copy", &purpose_yaml("root", None, "active")); // IDとファイル名の不一致・重複ID
    write(r, "broken", "id: [oops\n");
    write(
        r,
        "under-broken",
        &purpose_yaml("under-broken", Some("broken"), "active"),
    );

    let snap = load(r, None).unwrap();
    assert!(codes(snap.node("orphan").unwrap()).contains(&Code::ParentNotFound));
    assert!(codes(snap.node("loop-a").unwrap()).contains(&Code::Cycle));
    assert!(codes(snap.node("loop-b").unwrap()).contains(&Code::Cycle));
    assert_eq!(
        codes(snap.node("below-loop").unwrap()),
        vec![Code::ParentUnresolved]
    );
    let copy = codes(snap.node("copy").unwrap());
    assert!(copy.contains(&Code::IdFileMismatch) && copy.contains(&Code::DuplicateId));
    assert_eq!(codes(snap.node("broken").unwrap()), vec![Code::YamlSyntax]);
    assert_eq!(
        codes(snap.node("under-broken").unwrap()),
        vec![Code::ParentUnresolved]
    );
    assert!(snap.node("broken").unwrap().fields.is_none());
}

#[test]
fn broken_file_does_not_hide_others() {
    // 受け入れ条件22
    let dir = sample_copy();
    fs::write(purpose_path(dir.path(), "acme-jobs"), "this: is: broken").unwrap();
    let snap = load(dir.path(), Some(TODAY)).unwrap();
    assert!(snap.node("acme-corp").unwrap().fields.is_some());
    assert!(snap.node("acme-jobs").unwrap().fields.is_none());
    let leaf = snap.node("search-relevance-2026h2").unwrap();
    assert!(leaf.fields.is_some());
    assert!(!leaf.resolved);
    assert_eq!(leaf.effective, Effective::Unknown);
}

#[test]
fn multiple_roots_is_warning() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", &purpose_yaml("a", None, "active"));
    write(dir.path(), "b", &purpose_yaml("b", None, "active"));
    let snap = load(dir.path(), None).unwrap();
    assert_eq!(
        snap.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
        vec![Code::WorkspaceFileMissing, Code::MultipleRoots]
    );
}

#[test]
fn workspaces_are_isolated() {
    // 受け入れ条件15・16
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    init_workspace(a.path(), "A").unwrap();
    init_workspace(b.path(), "B").unwrap();
    write(
        a.path(),
        "shared-id",
        &purpose_yaml("shared-id", None, "active"),
    );
    write(
        b.path(),
        "shared-id",
        &purpose_yaml("shared-id", None, "active"),
    );
    write(
        b.path(),
        "child",
        &purpose_yaml("child", Some("only-in-a"), "active"),
    );
    write(
        a.path(),
        "only-in-a",
        &purpose_yaml("only-in-a", Some("shared-id"), "active"),
    );

    let before_a = fs::read(purpose_path(a.path(), "shared-id")).unwrap();
    let sa = load(a.path(), None).unwrap();
    let sb = load(b.path(), None).unwrap();
    assert_eq!(sa.name, "A");
    assert_eq!(sb.name, "B");
    assert!(sa.node("shared-id").unwrap().diagnostics.is_empty());
    assert!(sb.node("shared-id").unwrap().diagnostics.is_empty());
    assert!(codes(sb.node("child").unwrap()).contains(&Code::ParentNotFound));
    assert_eq!(
        fs::read(purpose_path(a.path(), "shared-id")).unwrap(),
        before_a
    );
}

#[test]
fn save_keeps_unknown_fields_and_writes_lf() {
    // 受け入れ条件21・23
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    let body = "owner: team-a\r\nschema_version: 1\r\nid: x\r\nparent: null\r\nname: X\r\npurpose: old\r\nstatus: active\r\nextra:\r\n  k: v\r\n";
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend(body.as_bytes());
    fs::create_dir_all(r.join("purposes")).unwrap();
    fs::write(purpose_path(r, "x"), bytes).unwrap();

    let snap = load(r, None).unwrap();
    let mut f = fields_of(&snap, "x");
    f.purpose = "new".into();
    let out = save(r, &snap, f).unwrap();

    let written = fs::read(purpose_path(r, "x")).unwrap();
    let text = String::from_utf8(written.clone()).unwrap();
    assert!(!written.starts_with(b"\xEF\xBB\xBF"));
    assert!(!text.contains('\r'));
    assert_eq!(
        text,
        "schema_version: 1\nid: x\nparent: null\nname: X\npurpose: new\nstatus: active\nowner: team-a\nextra:\n  k: v\n"
    );
    assert_eq!(out.hash, store::hash_bytes(&written));
}

#[test]
fn save_refuses_to_drop_comments_silently() {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    write(
        r,
        "x",
        &format!("# メモ\n{}", purpose_yaml("x", None, "active")),
    );
    let snap = load(r, None).unwrap();
    assert!(snap.node("x").unwrap().has_comments);
    let f = fields_of(&snap, "x");
    assert!(matches!(
        save(r, &snap, f.clone()),
        Err(SaveError::CommentsWouldBeLost)
    ));
    let hash = snap.node("x").unwrap().hash.clone();
    save_purpose(
        r,
        SaveRequest {
            fields: f,
            expected_hash: Some(hash),
            allow_comment_loss: true,
        },
    )
    .unwrap();
    assert!(
        !fs::read_to_string(purpose_path(r, "x"))
            .unwrap()
            .contains('#')
    );
}

#[test]
fn save_detects_external_change() {
    // 受け入れ条件6
    let dir = sample_copy();
    let r = dir.path();
    let snap = load(r, None).unwrap();
    let external = fs::read_to_string(purpose_path(r, "acme-jobs"))
        .unwrap()
        .replace("シンプルさを保つ", "外部エディタで変更");
    fs::write(purpose_path(r, "acme-jobs"), &external).unwrap();

    let mut f = fields_of(&snap, "acme-jobs");
    f.name = "GUIで変更".into();
    assert!(matches!(save(r, &snap, f), Err(SaveError::Conflict { .. })));
    assert_eq!(
        fs::read_to_string(purpose_path(r, "acme-jobs")).unwrap(),
        external
    );
}

#[test]
fn save_rejects_relational_errors() {
    let dir = sample_copy();
    let r = dir.path();
    let snap = load(r, None).unwrap();

    // 循環：acme-jobs の親を孫の job-matching にする
    let mut f = fields_of(&snap, "acme-jobs");
    f.parent = Some("job-matching".into());
    match save(r, &snap, f) {
        Err(SaveError::Validation { diagnostics }) => {
            assert!(diagnostics.iter().any(|d| d.code == Code::Cycle))
        }
        other => panic!("{other:?}"),
    }

    // 存在しない親
    let mut f = fields_of(&snap, "acme-jobs");
    f.id = "new-node".into();
    f.parent = Some("nowhere".into());
    match save_purpose(
        r,
        SaveRequest {
            fields: f,
            expected_hash: None,
            allow_comment_loss: false,
        },
    ) {
        Err(SaveError::Validation { diagnostics }) => {
            assert!(diagnostics.iter().any(|d| d.code == Code::ParentNotFound))
        }
        other => panic!("{other:?}"),
    }
    assert!(!purpose_path(r, "new-node").exists());

    // 入力値の誤り（日付）
    let mut f = fields_of(&snap, "acme-jobs");
    f.valid_from = Some("2026-02-30".into());
    assert!(matches!(
        save(r, &snap, f),
        Err(SaveError::Validation { .. })
    ));
}

#[test]
fn create_and_reject_duplicate() {
    let dir = sample_copy();
    let r = dir.path();
    let snap = load(r, None).unwrap();
    let mut f = fields_of(&snap, "search-experience");
    f.id = "apply-flow".into();
    f.name = "応募導線".into();
    f.parent = Some("job-matching".into());
    save_purpose(
        r,
        SaveRequest {
            fields: f.clone(),
            expected_hash: None,
            allow_comment_loss: false,
        },
    )
    .unwrap();
    let snap = load(r, None).unwrap();
    assert_eq!(
        snap.node("apply-flow").unwrap().ancestors,
        ["acme-corp", "acme-jobs", "job-matching"]
    );
    assert!(matches!(
        save_purpose(
            r,
            SaveRequest {
                fields: f,
                expected_hash: None,
                allow_comment_loss: false
            }
        ),
        Err(SaveError::AlreadyExists)
    ));
}

#[test]
fn save_rejects_invalid_id() {
    // 受け入れ条件20
    let dir = sample_copy();
    let snap = load(dir.path(), None).unwrap();
    for bad in ["Upper", "con", "a_b", "-x"] {
        let mut f = fields_of(&snap, "acme-jobs");
        f.id = bad.into();
        assert!(
            matches!(
                save_purpose(
                    dir.path(),
                    SaveRequest {
                        fields: f,
                        expected_hash: None,
                        allow_comment_loss: false
                    }
                ),
                Err(SaveError::Validation { .. })
            ),
            "{bad}"
        );
    }
}

#[test]
fn delete_only_leaves() {
    let dir = sample_copy();
    let r = dir.path();
    let snap = load(r, None).unwrap();
    let parent = snap.node("legacy-portal").unwrap();
    match delete_purpose(r, "legacy-portal", &parent.hash) {
        Err(SaveError::HasChildren { children }) => assert_eq!(children, ["legacy-data-migration"]),
        other => panic!("{other:?}"),
    }
    let leaf = snap.node("legacy-data-migration").unwrap();
    assert!(matches!(
        delete_purpose(r, "legacy-data-migration", "stale"),
        Err(SaveError::Conflict { .. })
    ));
    delete_purpose(r, "legacy-data-migration", &leaf.hash).unwrap();
    assert!(!purpose_path(r, "legacy-data-migration").exists());
}

#[test]
fn not_editable_when_file_has_errors() {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    write(
        r,
        "x",
        "schema_version: 1\nid: x\nparent: null\nname: X\npurpose: P\nstatus: completed\n",
    );
    let snap = load(r, None).unwrap();
    let hash = snap.node("x").unwrap().hash.clone();
    let f = PurposeFields {
        id: "x".into(),
        parent: None,
        name: "X".into(),
        purpose: "P".into(),
        rationale: None,
        principles: None,
        status: Status::Active,
        valid_from: None,
        valid_until: None,
        review_at: None,
    };
    assert!(matches!(
        save_purpose(
            r,
            SaveRequest {
                fields: f,
                expected_hash: Some(hash),
                allow_comment_loss: false
            }
        ),
        Err(SaveError::NotEditable { .. })
    ));
}

#[test]
fn not_a_workspace() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        load(dir.path(), None),
        Err(LoadError::NotAWorkspace(_))
    ));
    init_workspace(dir.path(), "New").unwrap();
    let snap = load(dir.path(), None).unwrap();
    assert_eq!(snap.name, "New");
    assert!(snap.nodes.is_empty());
}

#[test]
fn yaml_text_save_keeps_comments_and_formatting() {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    write(r, "x", &purpose_yaml("x", None, "active"));
    let before = read_purpose_text(r, "x").unwrap();
    let edited = format!(
        "# 直接編集したメモ\n{}",
        before.text.replace("x の目的", "変更した目的")
    );
    let out = save_purpose_text(r, "x", &edited, &before.hash).unwrap();
    assert_eq!(fs::read_to_string(purpose_path(r, "x")).unwrap(), edited);
    let snap = load(r, None).unwrap();
    assert_eq!(snap.node("x").unwrap().hash, out.hash);
    assert_eq!(fields_of(&snap, "x").purpose, "変更した目的");
}

#[test]
fn yaml_text_save_rejects_invalid_content() {
    let dir = sample_copy();
    let r = dir.path();
    let before = read_purpose_text(r, "acme-jobs").unwrap();
    for bad in [
        "id: [broken".to_string(),
        before.text.replace("id: acme-jobs", "id: other-id"),
        before
            .text
            .replace("parent: acme-corp", "parent: missing-parent"),
    ] {
        assert!(
            matches!(
                save_purpose_text(r, "acme-jobs", &bad, &before.hash),
                Err(SaveError::Validation { .. })
            ),
            "{bad}"
        );
    }
    assert_eq!(read_purpose_text(r, "acme-jobs").unwrap().text, before.text);
}

#[test]
fn yaml_text_save_detects_external_change() {
    let dir = sample_copy();
    let r = dir.path();
    let before = read_purpose_text(r, "acme-jobs").unwrap();
    let external = before
        .text
        .replace("シンプルさを保つ", "外部エディタで変更");
    fs::write(purpose_path(r, "acme-jobs"), &external).unwrap();
    let mine = before.text.replace("シンプルさを保つ", "GUIで変更");
    assert!(matches!(
        save_purpose_text(r, "acme-jobs", &mine, &before.hash),
        Err(SaveError::Conflict { .. })
    ));
    assert_eq!(
        fs::read_to_string(purpose_path(r, "acme-jobs")).unwrap(),
        external
    );
}

#[test]
fn yaml_text_can_fix_broken_file() {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    write(r, "x", "id: x\nname: [broken\n");
    let snap = load(r, None).unwrap();
    assert!(snap.node("x").unwrap().fields.is_none());
    let before = read_purpose_text(r, "x").unwrap();
    save_purpose_text(r, "x", &purpose_yaml("x", None, "active"), &before.hash).unwrap();
    assert!(load(r, None).unwrap().node("x").unwrap().fields.is_some());
}

#[test]
fn yaml_text_rejects_path_like_ids() {
    let dir = tempfile::tempdir().unwrap();
    assert!(read_purpose_text(dir.path(), "../workspace").is_err());
    assert!(save_purpose_text(dir.path(), "a/b", "x", "h").is_err());
    assert!(save_purpose_text(dir.path(), "C:x", "x", "h").is_err());
    assert!(read_purpose_text(dir.path(), "x:stream").is_err());
}

#[test]
fn yaml_text_create_uses_id_as_file_name() {
    let dir = sample_copy();
    let r = dir.path();
    let text = format!(
        "# 新しい目的\n{}",
        purpose_yaml("acme-new", Some("acme-jobs"), "active")
    );
    let out = create_purpose_text(r, &text).unwrap();
    assert_eq!(out.id, "acme-new");
    assert_eq!(
        fs::read_to_string(purpose_path(r, "acme-new")).unwrap(),
        text
    );
    let snap = load(r, None).unwrap();
    assert_eq!(snap.node("acme-new").unwrap().hash, out.hash);
    assert!(!snap.node("acme-new").unwrap().has_errors());
}

#[test]
fn yaml_text_create_rejects_bad_input() {
    let dir = sample_copy();
    let r = dir.path();
    let before = fs::read_dir(r.join("purposes")).unwrap().count();
    assert!(matches!(
        create_purpose_text(r, &purpose_yaml("acme-jobs", Some("acme-corp"), "active")),
        Err(SaveError::AlreadyExists)
    ));
    for bad in [
        "name: [broken".to_string(),
        "name: idなし\npurpose: x\n".to_string(),
        purpose_yaml("Bad_ID", None, "active"),
        purpose_yaml("orphan", Some("missing-parent"), "active"),
        "id: no-name\nparent: null\nstatus: active\n".to_string(),
    ] {
        assert!(
            matches!(
                create_purpose_text(r, &bad),
                Err(SaveError::Validation { .. })
            ),
            "{bad}"
        );
    }
    assert_eq!(fs::read_dir(r.join("purposes")).unwrap().count(), before);
}

#[test]
fn yaml_text_template_round_trips() {
    let fields = PurposeFields {
        id: "x".into(),
        parent: Some("p".into()),
        name: "名前".into(),
        purpose: "目的".into(),
        rationale: Some("".into()),
        principles: Some(vec!["原則".into(), "".into()]),
        status: Status::Active,
        valid_from: None,
        valid_until: None,
        review_at: None,
    };
    let text = purpose_text_template(fields).unwrap();
    assert!(
        text.contains("id: x\n") && text.contains("parent: p\n") && !text.contains("rationale")
    );
}
