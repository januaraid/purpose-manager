//! 目的ノードのデータモデル（仕様書 §4.2）。

use serde::{Deserialize, Serialize};
use yaml_rust2::Yaml;

pub const SCHEMA_VERSION: i64 = 1;

/// 出力時の既定のキー順序（仕様書 §4.6）。
pub const KNOWN_KEYS: &[&str] = &[
    "schema_version",
    "id",
    "parent",
    "name",
    "purpose",
    "rationale",
    "principles",
    "status",
    "valid_from",
    "valid_until",
    "review_at",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Active,
    Archived,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Active => "active",
            Status::Archived => "archived",
        }
    }
}

/// GUIとやり取りする既知フィールド。未知フィールドは含まない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurposeFields {
    pub id: String,
    pub parent: Option<String>,
    pub name: String,
    pub purpose: String,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub principles: Option<Vec<String>>,
    pub status: Status,
    #[serde(default)]
    pub valid_from: Option<String>,
    #[serde(default)]
    pub valid_until: Option<String>,
    #[serde(default)]
    pub review_at: Option<String>,
}

/// ファイル上の目的ノード。未知フィールドは元の順序で保持する。
#[derive(Debug, Clone, PartialEq)]
pub struct Purpose {
    pub schema_version: i64,
    pub fields: PurposeFields,
    pub extra: Vec<(Yaml, Yaml)>,
}

impl Purpose {
    pub fn new(fields: PurposeFields) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            fields,
            extra: Vec::new(),
        }
    }

    pub fn unknown_keys(&self) -> Vec<String> {
        self.extra
            .iter()
            .map(|(k, _)| match k {
                Yaml::String(s) => s.clone(),
                other => format!("{other:?}"),
            })
            .collect()
    }
}
