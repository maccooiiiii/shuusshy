//! Shuusshy license key format, shared by the app (verification) and the offline key tool in
//! `tools/license-keygen` (signing), so both sides always agree on the bytes.
//!
//! A key is `SHUU-` followed by Crockford base32 of `payload || signature`, in groups of five:
//! - payload (10 bytes): version (1) = 1, edition (1) = 1 Pro, serial (u32 BE),
//!   issued day (u16 BE), expiry day (u16 BE, 0 = never). Days count from 2020-01-01.
//! - signature (64 bytes): Ed25519 over [`DOMAIN`] || payload.
//!
//! Verification is offline: the app holds only the public key.

pub const DOMAIN: &[u8] = b"SHUUSSHY-LICENSE-V1";
pub const PREFIX: &str = "SHUU";
pub const PAYLOAD_LEN: usize = 10;
pub const SIG_LEN: usize = 64;
pub const VERSION: u8 = 1;
pub const EDITION_PRO: u8 = 1;

/// Days from 1970-01-01 to 2020-01-01.
const EPOCH_2020: i64 = 18_262;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Payload {
    pub edition: u8,
    pub serial: u32,
    /// Days since 2020-01-01.
    pub issued: u16,
    /// Days since 2020-01-01; 0 = never expires.
    pub expires: u16,
}

impl Payload {
    pub fn to_bytes(self) -> [u8; PAYLOAD_LEN] {
        let mut b = [0u8; PAYLOAD_LEN];
        b[0] = VERSION;
        b[1] = self.edition;
        b[2..6].copy_from_slice(&self.serial.to_be_bytes());
        b[6..8].copy_from_slice(&self.issued.to_be_bytes());
        b[8..10].copy_from_slice(&self.expires.to_be_bytes());
        b
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() != PAYLOAD_LEN || b[0] != VERSION {
            return None;
        }
        Some(Self {
            edition: b[1],
            serial: u32::from_be_bytes(b[2..6].try_into().ok()?),
            issued: u16::from_be_bytes(b[6..8].try_into().ok()?),
            expires: u16::from_be_bytes(b[8..10].try_into().ok()?),
        })
    }
}

/// The message that is signed.
pub fn signed_message(payload: &[u8; PAYLOAD_LEN]) -> Vec<u8> {
    [DOMAIN, payload.as_slice()].concat()
}

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn base32_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for &byte in data {
        acc = (acc << 8) | byte as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((acc >> bits) & 31) as usize] as char);
        }
        acc &= (1 << bits) - 1;
    }
    if bits > 0 {
        out.push(ALPHABET[((acc << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// Decodes Crockford base32, tolerant of case and the usual look-alikes (I/L = 1, O = 0).
pub fn base32_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.chars() {
        let c = match c.to_ascii_uppercase() {
            'I' | 'L' => '1',
            'O' => '0',
            c => c,
        };
        let v = ALPHABET.iter().position(|&a| a as char == c)? as u32;
        acc = (acc << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// `SHUU-XXXXX-XXXXX-…` from payload and signature.
pub fn format_key(payload: &[u8; PAYLOAD_LEN], sig: &[u8; SIG_LEN]) -> String {
    let body = base32_encode(&[payload.as_slice(), sig.as_slice()].concat());
    let groups: Vec<&str> = body
        .as_bytes()
        .chunks(5)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect();
    format!("{PREFIX}-{}", groups.join("-"))
}

/// Splits a typed or pasted key into payload and signature bytes. Ignores spaces, dashes,
/// line breaks and case.
pub fn split_key(text: &str) -> Option<([u8; PAYLOAD_LEN], [u8; SIG_LEN])> {
    let compact: String = text.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let upper = compact.to_ascii_uppercase();
    let body = upper.strip_prefix(PREFIX)?;
    let bytes = base32_decode(body)?;
    if bytes.len() != PAYLOAD_LEN + SIG_LEN {
        return None;
    }
    Some((
        bytes[..PAYLOAD_LEN].try_into().ok()?,
        bytes[PAYLOAD_LEN..].try_into().ok()?,
    ))
}

/// Days since 2020-01-01 for a Unix timestamp in seconds.
pub fn day_from_unix(secs: i64) -> i64 {
    secs.div_euclid(86_400) - EPOCH_2020
}

/// Calendar date (year, month, day) for a day number since 2020-01-01.
pub fn date_of_day(day: i64) -> (i64, u32, u32) {
    // Howard Hinnant's civil_from_days.
    let z = day + EPOCH_2020 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Day number since 2020-01-01 for a calendar date.
pub fn day_of_date(y: i64, m: u32, d: u32) -> i64 {
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468 - EPOCH_2020
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base32_round_trips() {
        for len in 0..80 {
            let data: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(
                base32_decode(&base32_encode(&data)).unwrap()[..len],
                data[..]
            );
        }
        assert_eq!(base32_decode("1o").unwrap(), base32_decode("I0").unwrap());
    }

    #[test]
    fn dates_round_trip() {
        assert_eq!(day_of_date(2020, 1, 1), 0);
        assert_eq!(date_of_day(0), (2020, 1, 1));
        assert_eq!(date_of_day(day_of_date(2026, 11, 14)), (2026, 11, 14));
        assert_eq!(date_of_day(day_of_date(2028, 2, 29)), (2028, 2, 29));
        assert_eq!(day_from_unix(1_577_836_800), 0); // 2020-01-01T00:00:00Z
    }

    #[test]
    fn key_text_round_trips() {
        let p = Payload {
            edition: EDITION_PRO,
            serial: 42,
            issued: 2509,
            expires: 0,
        }
        .to_bytes();
        let sig = [7u8; SIG_LEN];
        let key = format_key(&p, &sig);
        assert!(key.starts_with("SHUU-"));
        let messy = format!("  {}\n", key.to_lowercase().replace('-', " - "));
        assert_eq!(split_key(&messy), Some((p, sig)));
        assert_eq!(split_key("SHUU-ABCDE"), None);
        assert_eq!(split_key(&key.replacen("SHUU", "XXXX", 1)), None);
    }
}
