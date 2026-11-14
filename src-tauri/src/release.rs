//! Release metadata, the two-year validity period (spec Part 10) and the GitHub address.
//!
//! Official builds are compiled by the release workflow with these environment variables:
//! - `SHUUSSHY_RELEASE_CHANNEL` (e.g. `stable`) and `SHUUSSHY_RELEASE_DATE` (`YYYY-MM-DD`);
//! - `SHUUSSHY_RELEASE_SIG`: hex Ed25519 signature by the release key over
//!   `release_format::message(version, channel, date)`;
//! - `SHUUSSHY_GITHUB_REPO` (`owner/shuusshy`): enables update checks and the Releases link.
//!
//! The app verifies the signature with [`RELEASE_PUBLIC_KEY`] and expires two calendar years
//! after the release date: a discreet reminder 30 days before, a prominent warning 7 days
//! before, and on expiry audio processing and the virtual microphone stop (settings are kept,
//! nothing is uninstalled). Builds without valid metadata say "unofficial build" and do not
//! expire.
//!
//! This is transparent and best-effort: it uses the system clock, and anyone who can rebuild
//! or modify the program can remove it. There is deliberately no anti-tampering.

use ed25519_dalek::{Signature, VerifyingKey};
use serde::Serialize;

use crate::license_format::{date_of_day, day_from_unix, day_of_date};
use crate::release_format::{message, parse_date, plus_two_years, unhex};

/// Public half of `release-signing.key` (`tools/license-keygen release-init`).
pub const RELEASE_PUBLIC_KEY: [u8; 32] = [
    0xa6, 0x60, 0x05, 0xb2, 0xa1, 0x17, 0x92, 0xdc, 0x00, 0xc3, 0x8a, 0xba, 0x25, 0xc6, 0x76, 0x24,
    0xdd, 0xb1, 0x3d, 0xa2, 0x09, 0x9c, 0x78, 0x36, 0x1c, 0xfa, 0xac, 0x17, 0x18, 0x85, 0x16, 0xfe,
];

pub const GITHUB_REPO: Option<&str> = option_env!("SHUUSSHY_GITHUB_REPO");
const CHANNEL: Option<&str> = option_env!("SHUUSSHY_RELEASE_CHANNEL");
const RELEASE_DATE: Option<&str> = option_env!("SHUUSSHY_RELEASE_DATE");
const RELEASE_SIG: Option<&str> = option_env!("SHUUSSHY_RELEASE_SIG");

pub const REMINDER_DAYS: i64 = 30;
pub const WARNING_DAYS: i64 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Validity {
    /// No (valid) signed release metadata: a development or self-built copy. Never expires.
    Unofficial,
    Valid,
    /// 30 days or less left.
    Reminder,
    /// 7 days or less left.
    Warning,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub version: String,
    pub channel: Option<String>,
    pub release_date: Option<String>,
    pub expires: Option<String>,
    pub days_left: Option<i64>,
    pub validity: Validity,
    /// `owner/repo`, when the build knows its GitHub repository.
    pub repo: Option<String>,
    /// The official download page (GitHub Releases), when known.
    pub releases_url: Option<String>,
}

impl ReleaseInfo {
    pub fn expired(&self) -> bool {
        self.validity == Validity::Expired
    }
}

fn ymd((y, m, d): (i64, u32, u32)) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

/// Pure evaluation (tested); `today` counts days since 2020-01-01.
#[allow(clippy::too_many_arguments)]
pub fn evaluate(
    version: &str,
    channel: Option<&str>,
    date: Option<&str>,
    sig: Option<&str>,
    repo: Option<&str>,
    public_key: &[u8; 32],
    today: i64,
) -> ReleaseInfo {
    let repo = repo.filter(|r| valid_repo(r)).map(str::to_string);
    let mut info = ReleaseInfo {
        version: version.to_string(),
        channel: None,
        release_date: None,
        expires: None,
        days_left: None,
        validity: Validity::Unofficial,
        releases_url: repo
            .as_ref()
            .map(|r| format!("https://github.com/{r}/releases/latest")),
        repo,
    };
    let (Some(channel), Some(date), Some(sig)) = (channel, date, sig) else {
        return info;
    };
    let Some(released) = parse_date(date) else {
        return info;
    };
    let verified = unhex(sig)
        .and_then(|b| <[u8; 64]>::try_from(b).ok())
        .zip(VerifyingKey::from_bytes(public_key).ok())
        .is_some_and(|(sig, vk)| {
            vk.verify_strict(
                &message(version, channel, date),
                &Signature::from_bytes(&sig),
            )
            .is_ok()
        });
    if !verified {
        return info;
    }
    let expires = plus_two_years(released);
    let left = day_of_date(expires.0, expires.1, expires.2) - today;
    info.channel = Some(channel.to_string());
    info.release_date = Some(ymd(released));
    info.expires = Some(ymd(expires));
    info.days_left = Some(left.max(0));
    info.validity = match left {
        ..=0 => Validity::Expired,
        1..=WARNING_DAYS => Validity::Warning,
        l if l <= REMINDER_DAYS => Validity::Reminder,
        _ => Validity::Valid,
    };
    info
}

/// GitHub `owner/repo` with only the characters GitHub allows.
pub fn valid_repo(r: &str) -> bool {
    let mut parts = r.split('/');
    let ok = |p: &str| {
        !p.is_empty()
            && p.len() <= 100
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    matches!((parts.next(), parts.next(), parts.next()), (Some(a), Some(b), None) if ok(a) && ok(b))
}

/// Today as a day number since 2020-01-01 (system clock). Debug builds accept
/// `SHUUSSHY_FAKE_TODAY=YYYY-MM-DD` to test reminders and expiry.
pub fn today() -> i64 {
    #[cfg(debug_assertions)]
    if let Some((y, m, d)) = std::env::var("SHUUSSHY_FAKE_TODAY")
        .ok()
        .and_then(|s| parse_date(&s))
    {
        return day_of_date(y, m, d);
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    day_from_unix(secs)
}

/// This build's release information.
pub fn current(version: &str) -> ReleaseInfo {
    #[cfg(debug_assertions)]
    let overridden = (
        std::env::var("SHUUSSHY_RELEASE_CHANNEL").ok(),
        std::env::var("SHUUSSHY_RELEASE_DATE").ok(),
        std::env::var("SHUUSSHY_RELEASE_SIG").ok(),
    );
    #[cfg(debug_assertions)]
    let (channel, date, sig) = (
        overridden.0.as_deref().or(CHANNEL),
        overridden.1.as_deref().or(RELEASE_DATE),
        overridden.2.as_deref().or(RELEASE_SIG),
    );
    #[cfg(not(debug_assertions))]
    let (channel, date, sig) = (CHANNEL, RELEASE_DATE, RELEASE_SIG);
    evaluate(
        version,
        channel,
        date,
        sig,
        GITHUB_REPO,
        &RELEASE_PUBLIC_KEY,
        today(),
    )
}

/// Calendar date for a day number (for messages).
pub fn date_string(day: i64) -> String {
    ymd(date_of_day(day))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn signed(sk: &SigningKey, v: &str, c: &str, d: &str) -> String {
        crate::release_format::hex(&sk.sign(&message(v, c, d)).to_bytes())
    }

    #[test]
    fn signed_release_counts_down_to_expiry() {
        let sk = SigningKey::from_bytes(&[5; 32]);
        let pk = sk.verifying_key().to_bytes();
        let sig = signed(&sk, "1.0.0", "stable", "2027-01-01");
        let at = |y, m, d| {
            evaluate(
                "1.0.0",
                Some("stable"),
                Some("2027-01-01"),
                Some(&sig),
                Some("ian/shuusshy"),
                &pk,
                day_of_date(y, m, d),
            )
        };
        let v = at(2027, 6, 1);
        assert_eq!(v.validity, Validity::Valid);
        assert_eq!(v.expires.as_deref(), Some("2029-01-01"));
        assert_eq!(
            v.releases_url.as_deref(),
            Some("https://github.com/ian/shuusshy/releases/latest")
        );
        assert_eq!(at(2028, 12, 2).validity, Validity::Reminder); // 30 days left
        assert_eq!(at(2028, 12, 1).validity, Validity::Valid); // 31 days left
        assert_eq!(at(2028, 12, 25).validity, Validity::Warning); // 7 days left
        assert_eq!(at(2028, 12, 31).days_left, Some(1));
        assert_eq!(at(2029, 1, 1).validity, Validity::Expired);
        assert_eq!(at(2030, 1, 1).days_left, Some(0));
    }

    #[test]
    fn missing_or_forged_metadata_is_unofficial() {
        let sk = SigningKey::from_bytes(&[5; 32]);
        let pk = sk.verifying_key().to_bytes();
        let sig = signed(&sk, "1.0.0", "stable", "2027-01-01");
        let today = day_of_date(2040, 1, 1);
        // No metadata at all (development build).
        let dev = evaluate("1.0.0", None, None, None, None, &pk, today);
        assert_eq!(dev.validity, Validity::Unofficial);
        assert_eq!(dev.expires, None);
        // A changed date, version or key invalidates the signature.
        for (v, d) in [("1.0.0", "2039-01-01"), ("1.0.1", "2027-01-01")] {
            let r = evaluate(v, Some("stable"), Some(d), Some(&sig), None, &pk, today);
            assert_eq!(r.validity, Validity::Unofficial, "{v} {d}");
        }
        let other = SigningKey::from_bytes(&[6; 32]).verifying_key().to_bytes();
        assert_eq!(
            evaluate(
                "1.0.0",
                Some("stable"),
                Some("2027-01-01"),
                Some(&sig),
                None,
                &other,
                today
            )
            .validity,
            Validity::Unofficial
        );
        assert_eq!(
            evaluate(
                "1.0.0",
                Some("stable"),
                Some("2027-01-01"),
                Some("zz"),
                None,
                &pk,
                today
            )
            .validity,
            Validity::Unofficial
        );
    }

    #[test]
    fn repo_names_are_validated() {
        assert!(valid_repo("ian-musa/shuusshy"));
        assert!(!valid_repo("a/b/c"));
        assert!(!valid_repo("a"));
        assert!(!valid_repo("a/b?x=1"));
        assert!(!valid_repo("a/"));
    }
}
