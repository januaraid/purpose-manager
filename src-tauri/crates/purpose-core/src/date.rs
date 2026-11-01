//! `YYYY-MM-DD` 形式の日付（仕様書 §4.2、§4.7）。
//! 固定長のため、妥当な日付どうしは文字列の大小比較で前後を判定できる。

pub fn is_valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| -> Option<u32> {
        let part = &s[r];
        if part.bytes().all(|c| c.is_ascii_digit()) {
            part.parse().ok()
        } else {
            None
        }
    };
    let (Some(y), Some(m), Some(d)) = (digits(0..4), digits(5..7), digits(8..10)) else {
        return false;
    };
    if !(1..=12).contains(&m) || d == 0 {
        return false;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let max = match m {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    d <= max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_dates() {
        assert!(is_valid_date("2026-10-01"));
        assert!(is_valid_date("2028-02-29"));
        assert!(is_valid_date("2000-02-29"));
        assert!(!is_valid_date("2100-02-29"));
        assert!(!is_valid_date("2027-02-29"));
        assert!(!is_valid_date("2026-13-01"));
        assert!(!is_valid_date("2026-04-31"));
        assert!(!is_valid_date("2026-1-01"));
        assert!(!is_valid_date("2026/10/01"));
        assert!(!is_valid_date("2026-10-01T00:00:00"));
        assert!(!is_valid_date("２０２６-10-01"));
    }
}
