//! 目的YAMLの読み込みとファイル単位の検証（仕様書 §4.2、§4.7）。

use yaml_rust2::{Yaml, YamlLoader};

use crate::date::is_valid_date;
use crate::diagnostics::{Code, Diagnostic};
use crate::id::validate_id;
use crate::model::{KNOWN_KEYS, Purpose, PurposeFields, SCHEMA_VERSION, Status};

/// 壊れたファイルでも表示に使えるよう、読み取れた範囲の値を保持する。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawInfo {
    pub id: Option<String>,
    pub name: Option<String>,
    pub parent: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedPurpose {
    /// ファイル単位のエラーが一つもないときだけ `Some`。
    pub purpose: Option<Purpose>,
    pub raw: RawInfo,
    pub diagnostics: Vec<Diagnostic>,
}

/// BOMを取り除き、CRLFをLFにそろえる。UTF-8でなければ `None`。
pub fn decode(bytes: &[u8]) -> Option<String> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).ok()?;
    Some(text.replace("\r\n", "\n"))
}

pub fn parse_bytes(bytes: &[u8], file_stem: &str) -> ParsedPurpose {
    match decode(bytes) {
        Some(text) => parse_purpose(&text, file_stem),
        None => ParsedPurpose {
            purpose: None,
            raw: RawInfo::default(),
            diagnostics: vec![Diagnostic::error(
                Code::Encoding,
                "UTF-8として読み込めません。文字コードをUTF-8にしてください",
            )],
        },
    }
}

/// YAML文字列を一つのマッピングとして読み込む。
pub fn load_single_mapping(text: &str) -> Result<yaml_rust2::yaml::Hash, Diagnostic> {
    let docs = YamlLoader::load_from_str(text)
        .map_err(|e| Diagnostic::error(Code::YamlSyntax, format!("YAMLの構文エラー: {e}")))?;
    match docs.len() {
        0 => Err(Diagnostic::error(Code::NotAMapping, "ファイルが空です")),
        1 => match docs.into_iter().next().unwrap() {
            Yaml::Hash(h) => Ok(h),
            _ => Err(Diagnostic::error(
                Code::NotAMapping,
                "トップレベルがマッピング（key: value）ではありません",
            )),
        },
        _ => Err(Diagnostic::error(
            Code::MultipleDocuments,
            "1ファイルに複数のYAMLドキュメントがあります",
        )),
    }
}

pub fn parse_purpose(text: &str, file_stem: &str) -> ParsedPurpose {
    let mut diags = Vec::new();
    let mut raw = RawInfo::default();

    if let Err(msg) = validate_id(file_stem) {
        diags.push(Diagnostic::error(
            Code::InvalidFileName,
            format!("ファイル名がID規則に合いません: {msg}"),
        ));
    }

    let map = match load_single_mapping(text) {
        Ok(m) => m,
        Err(d) => {
            diags.push(d);
            return ParsedPurpose {
                purpose: None,
                raw,
                diagnostics: diags,
            };
        }
    };

    let get = |key: &str| map.get(&Yaml::String(key.to_string()));

    // schema_version
    let schema_version = match get("schema_version") {
        None => {
            diags.push(
                Diagnostic::error(Code::MissingField, "schema_version がありません")
                    .field("schema_version"),
            );
            None
        }
        Some(Yaml::Integer(v)) if *v == SCHEMA_VERSION => Some(*v),
        Some(Yaml::Integer(v)) => {
            diags.push(
                Diagnostic::error(
                    Code::UnsupportedSchemaVersion,
                    format!("schema_version {v} には対応していません（対応: {SCHEMA_VERSION}）"),
                )
                .field("schema_version"),
            );
            None
        }
        Some(_) => {
            diags.push(
                Diagnostic::error(Code::InvalidType, "schema_version は整数にしてください")
                    .field("schema_version"),
            );
            None
        }
    };

    // id
    let id = required_string(get("id"), "id", &mut diags);
    if let Some(id) = &id {
        raw.id = Some(id.clone());
        match validate_id(id) {
            Err(msg) => diags.push(Diagnostic::error(Code::InvalidId, msg).field("id")),
            Ok(()) if id != file_stem => diags.push(
                Diagnostic::error(
                    Code::IdFileMismatch,
                    format!("ID「{id}」とファイル名「{file_stem}.yaml」が一致しません"),
                )
                .field("id"),
            ),
            Ok(()) => {}
        }
    }

    // parent（キー自体は必須。値はIDかnull）
    let parent = match get("parent") {
        None => {
            diags.push(
                Diagnostic::error(Code::MissingField, "parent がありません（ルートは null）")
                    .field("parent"),
            );
            None
        }
        Some(Yaml::Null) => Some(None),
        Some(Yaml::String(p)) => {
            raw.parent = Some(p.clone());
            match validate_id(p) {
                Ok(()) => Some(Some(p.clone())),
                Err(msg) => {
                    diags.push(
                        Diagnostic::error(
                            Code::InvalidParent,
                            format!("parent のIDが不正です: {msg}"),
                        )
                        .field("parent"),
                    );
                    None
                }
            }
        }
        Some(Yaml::Array(_)) => {
            diags.push(
                Diagnostic::error(
                    Code::InvalidParent,
                    "親目的は一つだけ指定できます（配列は使えません）",
                )
                .field("parent"),
            );
            None
        }
        Some(_) => {
            diags.push(
                Diagnostic::error(Code::InvalidParent, "parent は親のIDか null にしてください")
                    .field("parent"),
            );
            None
        }
    };

    let name = required_string(get("name"), "name", &mut diags);
    raw.name = name.clone();
    let purpose = required_string(get("purpose"), "purpose", &mut diags);
    let rationale = optional_string(get("rationale"), "rationale", &mut diags);
    let principles = optional_string_list(get("principles"), "principles", &mut diags);

    let status = match get("status") {
        None => {
            diags
                .push(Diagnostic::error(Code::MissingField, "status がありません").field("status"));
            None
        }
        Some(Yaml::String(s)) if s == "active" => Some(Status::Active),
        Some(Yaml::String(s)) if s == "archived" => Some(Status::Archived),
        Some(Yaml::String(s)) if s == "completed" => {
            diags.push(
                Diagnostic::error(Code::InvalidStatus, "status に completed は使えません。目的は達成判定を持たないため、active か archived にしてください")
                    .field("status"),
            );
            None
        }
        Some(_) => {
            diags.push(
                Diagnostic::error(
                    Code::InvalidStatus,
                    "status は active か archived にしてください",
                )
                .field("status"),
            );
            None
        }
    };

    let valid_from = optional_date(get("valid_from"), "valid_from", &mut diags);
    let valid_until = optional_date(get("valid_until"), "valid_until", &mut diags);
    let review_at = optional_date(get("review_at"), "review_at", &mut diags);
    if let (Some(Some(from)), Some(Some(until))) = (&valid_from, &valid_until)
        && from > until
    {
        diags.push(
            Diagnostic::error(
                Code::InvalidPeriod,
                format!("valid_from（{from}）が valid_until（{until}）より後になっています"),
            )
            .field("valid_from"),
        );
    }

    let extra: Vec<(Yaml, Yaml)> = map
        .iter()
        .filter(|(k, _)| !matches!(k, Yaml::String(s) if KNOWN_KEYS.contains(&s.as_str())))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if extra
        .iter()
        .any(|(k, v)| contains_unsupported(k) || contains_unsupported(v))
    {
        diags.push(Diagnostic::error(
            Code::UnsupportedValue,
            "アンカー・エイリアスなど、保存できない値が含まれています",
        ));
    }

    let purpose = match (
        schema_version,
        id,
        parent,
        name,
        purpose,
        rationale,
        principles,
        status,
        valid_from,
        valid_until,
        review_at,
    ) {
        (
            Some(sv),
            Some(id),
            Some(parent),
            Some(name),
            Some(purpose),
            Some(rationale),
            Some(principles),
            Some(status),
            Some(vf),
            Some(vu),
            Some(ra),
        ) if !crate::diagnostics::has_errors(&diags) => Some(Purpose {
            schema_version: sv,
            fields: PurposeFields {
                id,
                parent,
                name,
                purpose,
                rationale,
                principles,
                status,
                valid_from: vf,
                valid_until: vu,
                review_at: ra,
            },
            extra,
        }),
        _ => None,
    };

    ParsedPurpose {
        purpose,
        raw,
        diagnostics: diags,
    }
}

fn contains_unsupported(y: &Yaml) -> bool {
    match y {
        Yaml::Alias(_) | Yaml::BadValue => true,
        Yaml::Array(a) => a.iter().any(contains_unsupported),
        Yaml::Hash(h) => h
            .iter()
            .any(|(k, v)| contains_unsupported(k) || contains_unsupported(v)),
        _ => false,
    }
}

fn required_string(v: Option<&Yaml>, field: &str, diags: &mut Vec<Diagnostic>) -> Option<String> {
    match v {
        Some(Yaml::String(s)) if !s.trim().is_empty() => Some(s.clone()),
        Some(Yaml::String(_)) | None | Some(Yaml::Null) => {
            diags.push(
                Diagnostic::error(Code::MissingField, format!("{field} は必須です")).field(field),
            );
            None
        }
        Some(_) => {
            diags.push(
                Diagnostic::error(Code::InvalidType, format!("{field} は文字列にしてください"))
                    .field(field),
            );
            None
        }
    }
}

/// 外側の `Option` は「値として正しく読めたか」、内側は「値があるか」。
fn optional_string(
    v: Option<&Yaml>,
    field: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<Option<String>> {
    match v {
        None | Some(Yaml::Null) => Some(None),
        Some(Yaml::String(s)) => Some(Some(s.clone())),
        Some(_) => {
            diags.push(
                Diagnostic::error(Code::InvalidType, format!("{field} は文字列にしてください"))
                    .field(field),
            );
            None
        }
    }
}

fn optional_string_list(
    v: Option<&Yaml>,
    field: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<Option<Vec<String>>> {
    match v {
        None | Some(Yaml::Null) => Some(None),
        Some(Yaml::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Yaml::String(s) => out.push(s.clone()),
                    _ => {
                        diags.push(
                            Diagnostic::error(
                                Code::InvalidType,
                                format!("{field} の各項目は文字列にしてください"),
                            )
                            .field(field),
                        );
                        return None;
                    }
                }
            }
            Some(Some(out))
        }
        Some(_) => {
            diags.push(
                Diagnostic::error(
                    Code::InvalidType,
                    format!("{field} は文字列の配列にしてください"),
                )
                .field(field),
            );
            None
        }
    }
}

fn optional_date(
    v: Option<&Yaml>,
    field: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<Option<String>> {
    match v {
        None | Some(Yaml::Null) => Some(None),
        Some(Yaml::String(s)) if is_valid_date(s) => Some(Some(s.clone())),
        Some(_) => {
            diags.push(
                Diagnostic::error(
                    Code::InvalidDate,
                    format!("{field} は YYYY-MM-DD 形式の実在する日付にしてください"),
                )
                .field(field),
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Code;

    const VALID: &str = "schema_version: 1
id: acme-jobs
parent: acme-corp
name: Acme Jobs
purpose: 求職者が自分に合った仕事に出会える状態をつくる。
rationale: >
  求人情報の量は増えたが、
  自分に合う仕事を見つけにくい。
principles:
  - 求職者を第一に考える
  - シンプルさを保つ
status: active
valid_from: 2026-10-01
valid_until: 2027-03-31
review_at: 2027-02-28
";

    fn codes(p: &ParsedPurpose) -> Vec<Code> {
        p.diagnostics.iter().map(|d| d.code).collect()
    }

    #[test]
    fn parses_valid_file() {
        let p = parse_purpose(VALID, "acme-jobs");
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        let f = p.purpose.unwrap().fields;
        assert_eq!(f.parent.as_deref(), Some("acme-corp"));
        assert_eq!(f.status, Status::Active);
        assert_eq!(f.valid_from.as_deref(), Some("2026-10-01"));
        assert_eq!(f.principles.unwrap().len(), 2);
        assert_eq!(
            f.rationale.unwrap(),
            "求人情報の量は増えたが、 自分に合う仕事を見つけにくい。\n"
        );
    }

    #[test]
    fn rationale_is_optional() {
        let text = "schema_version: 1\nid: a\nparent: null\nname: A\npurpose: P\nstatus: active\n";
        let p = parse_purpose(text, "a");
        assert!(p.diagnostics.is_empty());
        let f = p.purpose.unwrap().fields;
        assert_eq!(f.rationale, None);
        assert_eq!(f.parent, None);
    }

    #[test]
    fn rejects_completed_status() {
        let text = VALID.replace("status: active", "status: completed");
        let p = parse_purpose(&text, "acme-jobs");
        assert!(p.purpose.is_none());
        assert_eq!(codes(&p), vec![Code::InvalidStatus]);
    }

    #[test]
    fn detects_id_file_mismatch() {
        let p = parse_purpose(VALID, "acme-other");
        assert_eq!(codes(&p), vec![Code::IdFileMismatch]);
        assert_eq!(p.raw.id.as_deref(), Some("acme-jobs"));
    }

    #[test]
    fn detects_invalid_id_and_file_name() {
        let text = VALID.replace("id: acme-jobs", "id: Acme-Jobs");
        let p = parse_purpose(&text, "Acme-Jobs");
        assert!(codes(&p).contains(&Code::InvalidId));
        assert!(codes(&p).contains(&Code::InvalidFileName));
    }

    #[test]
    fn rejects_multiple_parents() {
        let text = VALID.replace("parent: acme-corp", "parent: [a, b]");
        let p = parse_purpose(&text, "acme-jobs");
        assert_eq!(codes(&p), vec![Code::InvalidParent]);
    }

    #[test]
    fn requires_parent_key() {
        let text = VALID.replace("parent: acme-corp\n", "");
        assert_eq!(
            codes(&parse_purpose(&text, "acme-jobs")),
            vec![Code::MissingField]
        );
    }

    #[test]
    fn detects_bad_dates_and_period() {
        let text = VALID.replace("2027-02-28", "2027-02-30");
        assert_eq!(
            codes(&parse_purpose(&text, "acme-jobs")),
            vec![Code::InvalidDate]
        );
        let text = VALID.replace("valid_from: 2026-10-01", "valid_from: 2027-10-01");
        assert_eq!(
            codes(&parse_purpose(&text, "acme-jobs")),
            vec![Code::InvalidPeriod]
        );
        let text = VALID.replace("valid_from: 2026-10-01", "valid_from: 20261001");
        assert_eq!(
            codes(&parse_purpose(&text, "acme-jobs")),
            vec![Code::InvalidDate]
        );
    }

    #[test]
    fn detects_syntax_error() {
        let p = parse_purpose("id: [unclosed\n", "x");
        assert_eq!(codes(&p), vec![Code::YamlSyntax]);
    }

    #[test]
    fn detects_unsupported_schema_version() {
        let text = VALID.replace("schema_version: 1", "schema_version: 2");
        assert_eq!(
            codes(&parse_purpose(&text, "acme-jobs")),
            vec![Code::UnsupportedSchemaVersion]
        );
    }

    #[test]
    fn collects_multiple_errors() {
        let text = "schema_version: 1\nid: a\nparent: null\nname: 3\nstatus: done\n";
        let c = codes(&parse_purpose(text, "a"));
        assert!(c.contains(&Code::InvalidType));
        assert!(c.contains(&Code::MissingField));
        assert!(c.contains(&Code::InvalidStatus));
    }

    #[test]
    fn keeps_unknown_fields_in_order() {
        let text = format!("{VALID}owner: someone\ntags:\n  - x\n");
        let p = parse_purpose(&text, "acme-jobs").purpose.unwrap();
        assert_eq!(p.unknown_keys(), vec!["owner", "tags"]);
    }

    #[test]
    fn decodes_bom_and_crlf() {
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend(VALID.replace('\n', "\r\n").as_bytes());
        let p = parse_bytes(&bytes, "acme-jobs");
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        assert_eq!(
            p.purpose.unwrap().fields.rationale.unwrap(),
            "求人情報の量は増えたが、 自分に合う仕事を見つけにくい。\n"
        );
    }

    #[test]
    fn rejects_non_utf8() {
        let p = parse_bytes(&[0x82, 0xa0, 0xff], "a");
        assert_eq!(codes(&p), vec![Code::Encoding]);
    }
}
