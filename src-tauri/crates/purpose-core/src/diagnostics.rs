//! 検証結果（仕様書 §4.7 のエラー・警告・情報）。

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    // ファイル単位のエラー
    Encoding,
    YamlSyntax,
    NotAMapping,
    MultipleDocuments,
    MissingField,
    InvalidType,
    InvalidId,
    InvalidFileName,
    IdFileMismatch,
    InvalidParent,
    InvalidStatus,
    InvalidDate,
    InvalidPeriod,
    UnsupportedSchemaVersion,
    UnsupportedValue,
    // Workspace全体で判定するエラー
    DuplicateId,
    ParentNotFound,
    Cycle,
    // 警告
    ParentUnresolved,
    MultipleRoots,
    PeriodOutsideParent,
    ReviewOverdue,
    WorkspaceFileMissing,
    WorkspaceFileInvalid,
    // 情報
    InheritedInactive,
    UnknownFields,
    HasComments,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Code,
    pub message: String,
    /// 対象のファイル名（`purposes/` 内）。Workspace全体の診断では `None`。
    pub file: Option<String>,
    pub field: Option<String>,
}

impl Diagnostic {
    pub fn new(severity: Severity, code: Code, message: impl Into<String>) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            file: None,
            field: None,
        }
    }
    pub fn error(code: Code, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, code, message)
    }
    pub fn warning(code: Code, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, code, message)
    }
    pub fn info(code: Code, message: impl Into<String>) -> Self {
        Self::new(Severity::Info, code, message)
    }
    pub fn field(mut self, field: &str) -> Self {
        self.field = Some(field.to_string());
        self
    }
    pub fn file(mut self, file: &str) -> Self {
        self.file = Some(file.to_string());
        self
    }
}

pub fn has_errors(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}
