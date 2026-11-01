//! 目的IDの規則（仕様書 §4.1）。

pub const MAX_ID_LEN: usize = 64;

const WINDOWS_RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// IDが規則に合っていれば `Ok(())`、違反していれば理由を返す。
pub fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("IDが空です".into());
    }
    if id.len() > MAX_ID_LEN {
        return Err(format!("IDは{MAX_ID_LEN}文字以内にしてください"));
    }
    if let Some(c) = id
        .chars()
        .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-'))
    {
        return Err(if c.is_ascii_uppercase() {
            "IDに大文字は使えません（小文字英数字とハイフンのみ）".into()
        } else {
            format!("IDに使えない文字「{c}」が含まれています（小文字英数字とハイフンのみ）")
        });
    }
    if id.starts_with('-') || id.ends_with('-') || id.contains("--") {
        return Err("ハイフンは先頭・末尾・連続で使えません".into());
    }
    if WINDOWS_RESERVED.contains(&id) {
        return Err(format!("「{id}」はWindowsの予約名のため使えません"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_ids() {
        for id in [
            "a",
            "acme-corp",
            "job-matching",
            "x1",
            "2027h1-goal",
            &"a".repeat(64),
        ] {
            assert!(validate_id(id).is_ok(), "{id}");
        }
    }

    #[test]
    fn rejects_invalid_ids() {
        for id in [
            "",
            "Acme",
            "acme_corp",
            "acme corp",
            "-a",
            "a-",
            "a--b",
            "日本語",
            "a.b",
            "con",
            "nul",
            "lpt9",
            &"a".repeat(65),
        ] {
            assert!(validate_id(id).is_err(), "{id}");
        }
    }

    #[test]
    fn reserved_name_only_matches_exactly() {
        assert!(validate_id("console").is_ok());
        assert!(validate_id("com10").is_ok());
    }
}
