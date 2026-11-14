//! Signed release metadata format, shared by the app (verification) and
//! `tools/license-keygen` (signing in the release workflow).
//!
//! An official build embeds, at compile time, its release channel, release date and an
//! Ed25519 signature over [`message`]. The app verifies the signature against the release
//! public key and derives the expiry date: two calendar years after the release date
//! (spec Part 10). Builds without valid metadata are "unofficial" and show that openly.

pub const DOMAIN: &str = "SHUUSSHY-RELEASE-V1";

/// The exact bytes that are signed.
pub fn message(version: &str, channel: &str, release_date: &str) -> Vec<u8> {
    format!("{DOMAIN}|{version}|{channel}|{release_date}").into_bytes()
}

/// Parses `YYYY-MM-DD`.
pub fn parse_date(s: &str) -> Option<(i64, u32, u32)> {
    let mut it = s.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    if it.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) || y < 2020 {
        return None;
    }
    Some((y, m, d))
}

/// Two calendar years later; 29 February becomes 28 February.
pub fn plus_two_years((y, m, d): (i64, u32, u32)) -> (i64, u32, u32) {
    let leap = |y: i64| (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let ny = y + 2;
    if m == 2 && d == 29 && !leap(ny) {
        (ny, 2, 28)
    } else {
        (ny, m, d)
    }
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_two_year_validity() {
        assert_eq!(parse_date("2027-01-01"), Some((2027, 1, 1)));
        assert_eq!(parse_date("2027-13-01"), None);
        assert_eq!(parse_date("27-01-01"), None);
        assert_eq!(parse_date("2027-01-01-1"), None);
        // Spec examples: 1.0.0 on 2027-01-01 expires 2029-01-01; 1.1.0 on 2027-07-01 → 2029-07-01.
        assert_eq!(plus_two_years((2027, 1, 1)), (2029, 1, 1));
        assert_eq!(plus_two_years((2027, 7, 1)), (2029, 7, 1));
        assert_eq!(plus_two_years((2028, 2, 29)), (2030, 2, 28));
    }

    #[test]
    fn hex_round_trips() {
        let b = [0u8, 1, 0xab, 0xff];
        assert_eq!(unhex(&hex(&b)).unwrap(), b);
        assert_eq!(unhex("abc"), None);
        assert_eq!(unhex("zz"), None);
    }
}
