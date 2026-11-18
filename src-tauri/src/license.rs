//! Shuusshy Pro licensing: offline Ed25519-signed keys (format in `license_format.rs`).
//!
//! Free: RNNoise noise cancellation, the virtual microphone, live test, gains, gate.
//! Pro (key required): DeepFilterNet, voice isolation, and the presets built on them.
//!
//! Verification is local and needs no account or network. Shuusshy is open source, so the
//! check stops casual key sharing, not someone who rebuilds the app without it.

use ed25519_dalek::{Signature, VerifyingKey};
use serde::Serialize;

use crate::license_format::{
    EDITION_PRO, Payload, date_of_day, day_from_unix, signed_message, split_key,
};

/// Public half of the signing key created with `tools/license-keygen init`.
pub const PUBLIC_KEY: [u8; 32] = [
    0x2f, 0xfe, 0xd9, 0xb7, 0x8c, 0x48, 0x10, 0x74, 0x14, 0xc3, 0x1c, 0x87, 0xa1, 0xa0, 0x7a, 0xc7,
    0xf6, 0xea, 0xb7, 0x0a, 0xcc, 0x4e, 0xdf, 0x66, 0xe7, 0xc1, 0x8f, 0x2a, 0x33, 0x06, 0xd9, 0x66,
];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInfo {
    /// Master keys never expire; Pro keys have an expiry date. Both unlock the same features.
    pub kind: LicenseKind,
    pub serial: u32,
    /// YYYY-MM-DD.
    pub issued: String,
    /// YYYY-MM-DD, or None for a perpetual key.
    pub expires: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LicenseKind {
    Master,
    Pro,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum LicenseStatus {
    Free,
    Pro(LicenseInfo),
    /// A stored key that no longer works (expired). Pro features are off.
    Expired(LicenseInfo),
}

impl LicenseStatus {
    pub fn is_pro(&self) -> bool {
        matches!(self, Self::Pro(_))
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum LicenseError {
    #[error(
        "That doesn't look like a Shuusshy key. Keys start with SHUU- (copy and paste the whole key)."
    )]
    Malformed,
    #[error("This key is not valid for Shuusshy. Check that it was copied completely.")]
    BadSignature,
    #[error("This key is for a different edition of Shuusshy.")]
    WrongEdition,
    #[error("This key expired on {0}.")]
    Expired(String),
}

fn ymd(day: i64) -> String {
    let (y, m, d) = date_of_day(day);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Today as a day number since 2020-01-01.
pub fn today() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    day_from_unix(secs)
}

/// Checks a key against `public_key` on day `today`.
pub fn verify_with(
    key: &str,
    public_key: &[u8; 32],
    today: i64,
) -> Result<LicenseInfo, LicenseError> {
    let (payload_bytes, sig) = split_key(key).ok_or(LicenseError::Malformed)?;
    let vk = VerifyingKey::from_bytes(public_key).map_err(|_| LicenseError::BadSignature)?;
    vk.verify_strict(
        &signed_message(&payload_bytes),
        &Signature::from_bytes(&sig),
    )
    .map_err(|_| LicenseError::BadSignature)?;
    let p = Payload::from_bytes(&payload_bytes).ok_or(LicenseError::Malformed)?;
    if p.edition != EDITION_PRO {
        return Err(LicenseError::WrongEdition);
    }
    let info = LicenseInfo {
        kind: if p.expires == 0 {
            LicenseKind::Master
        } else {
            LicenseKind::Pro
        },
        serial: p.serial,
        issued: ymd(p.issued as i64),
        expires: (p.expires != 0).then(|| ymd(p.expires as i64)),
    };
    if p.expires != 0 && today > p.expires as i64 {
        return Err(LicenseError::Expired(ymd(p.expires as i64)));
    }
    Ok(info)
}

pub fn verify(key: &str) -> Result<LicenseInfo, LicenseError> {
    verify_with(key, &PUBLIC_KEY, today())
}

/// Status of a stored key (None = no key).
pub fn status_of(stored: Option<&str>) -> LicenseStatus {
    let Some(key) = stored else {
        return LicenseStatus::Free;
    };
    match verify(key) {
        Ok(info) => LicenseStatus::Pro(info),
        Err(LicenseError::Expired(_)) => {
            // Show which key expired; the signature was already checked.
            match verify_with(key, &PUBLIC_KEY, 0) {
                Ok(info) => LicenseStatus::Expired(info),
                Err(_) => LicenseStatus::Free,
            }
        }
        Err(_) => LicenseStatus::Free,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license_format::{PAYLOAD_LEN, format_key};
    use ed25519_dalek::{Signer, SigningKey};

    fn issue(sk: &SigningKey, edition: u8, expires: u16) -> String {
        let p: [u8; PAYLOAD_LEN] = Payload {
            edition,
            serial: 7,
            issued: 2509,
            expires,
        }
        .to_bytes();
        format_key(&p, &sk.sign(&signed_message(&p)).to_bytes())
    }

    #[test]
    fn valid_key_verifies_and_reports_dates() {
        let sk = SigningKey::from_bytes(&[9; 32]);
        let pk = sk.verifying_key().to_bytes();
        let info = verify_with(&issue(&sk, EDITION_PRO, 0), &pk, 2600).unwrap();
        assert_eq!(info.serial, 7);
        assert_eq!(info.issued, "2026-11-14");
        assert_eq!(info.expires, None);
    }

    #[test]
    fn keys_without_expiry_are_master_keys_and_others_pro_keys() {
        let sk = SigningKey::from_bytes(&[9; 32]);
        let pk = sk.verifying_key().to_bytes();
        let master = verify_with(&issue(&sk, EDITION_PRO, 0), &pk, 2600).unwrap();
        assert_eq!(master.kind, LicenseKind::Master);
        let pro = verify_with(&issue(&sk, EDITION_PRO, 3000), &pk, 2600).unwrap();
        assert_eq!(pro.kind, LicenseKind::Pro);
        assert!(pro.expires.is_some());
        // The app receives the kind as "master" / "pro".
        let json = serde_json::to_value(LicenseStatus::Pro(master)).unwrap();
        assert_eq!(json["kind"], "master");
        assert_eq!(json["state"], "pro");
    }

    #[test]
    fn rejects_tampered_foreign_expired_and_garbage_keys() {
        let sk = SigningKey::from_bytes(&[9; 32]);
        let pk = sk.verifying_key().to_bytes();
        let key = issue(&sk, EDITION_PRO, 0);

        // Change one character of the payload.
        let mut chars: Vec<char> = key.chars().collect();
        chars[6] = if chars[6] == 'A' { 'B' } else { 'A' };
        let tampered: String = chars.into_iter().collect();
        assert_eq!(
            verify_with(&tampered, &pk, 2600),
            Err(LicenseError::BadSignature)
        );

        // Signed by someone else.
        let other = SigningKey::from_bytes(&[3; 32]);
        assert_eq!(
            verify_with(&issue(&other, EDITION_PRO, 0), &pk, 2600),
            Err(LicenseError::BadSignature)
        );

        // Expired (expiry day 2600 = 2027-02-13).
        assert_eq!(
            verify_with(&issue(&sk, EDITION_PRO, 2600), &pk, 2601),
            Err(LicenseError::Expired("2027-02-13".into()))
        );
        assert!(verify_with(&issue(&sk, EDITION_PRO, 2600), &pk, 2600).is_ok());

        assert_eq!(
            verify_with(&issue(&sk, 9, 0), &pk, 2600),
            Err(LicenseError::WrongEdition)
        );
        assert_eq!(verify_with("hello", &pk, 0), Err(LicenseError::Malformed));
    }

    #[test]
    fn no_key_is_free() {
        assert_eq!(status_of(None), LicenseStatus::Free);
        assert_eq!(status_of(Some("SHUU-NOPE")), LicenseStatus::Free);
    }
}
