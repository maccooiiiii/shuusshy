//! Offline Shuusshy license tool. Keep the signing key private and backed up: anyone holding it
//! can issue keys, and losing it means existing keys can only be verified, never reissued.
//!
//! ```text
//! cargo run --release -- init                         # create the signing key (once)
//! cargo run --release -- pubkey                       # print the public key for the app
//! cargo run --release -- issue-pro <serial> [YYYY-MM-DD]  # Pro key; expires in two years
//!                                                          # unless a date is given
//! cargo run --release -- issue-master <serial>           # master key: never expires
//! cargo run --release -- issue <serial> [YYYY-MM-DD]      # older form (no record kept)
//!
//! issue-pro and issue-master refuse a serial that is already in the record and append the new
//! key to `issued-keys.txt` (next to the signing key) as
//! `KEY<TAB>serial N<TAB>tag pro|master<TAB>issued YYYY-MM-DD<TAB>expires YYYY-MM-DD|never`.
//! Both kinds unlock the same features; the app names them "Master key" and "Pro key".
//!
//! cargo run --release -- release-init                 # create the release signing key (once)
//! cargo run --release -- release-pubkey               # public key for src-tauri/src/release.rs
//! cargo run --release -- release-sign <version> <channel> <YYYY-MM-DD>   # hex signature
//! ```
//!
//! Keys live outside the repository, by default in `%USERPROFILE%\.shuusshy\`:
//! `license-signing.key` (override with SHUUSSHY_SIGNING_KEY) and `release-signing.key`
//! (override with SHUUSSHY_RELEASE_KEY; the release workflow passes a GitHub secret).

#[path = "../../../src-tauri/src/license_format.rs"]
#[allow(dead_code)]
mod format;

#[path = "../../../src-tauri/src/release_format.rs"]
#[allow(dead_code)]
mod release;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signer, SigningKey};
use format::{
    EDITION_PRO, Payload, date_of_day, day_from_unix, day_of_date, format_key, signed_message,
};
use rand_core::OsRng;

fn key_path() -> PathBuf {
    if let Some(p) = std::env::var_os("SHUUSSHY_SIGNING_KEY") {
        return p.into();
    }
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .expect("no home directory");
    PathBuf::from(home)
        .join(".shuusshy")
        .join("license-signing.key")
}

fn release_key_path() -> PathBuf {
    if let Some(p) = std::env::var_os("SHUUSSHY_RELEASE_KEY") {
        return p.into();
    }
    key_path().with_file_name("release-signing.key")
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn load() -> SigningKey {
    load_from(key_path())
}

fn load_from(path: PathBuf) -> SigningKey {
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e} (run `init` first)", path.display()));
    let bytes: Vec<u8> = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&text.trim()[i..i + 2], 16).expect("bad key file"))
        .collect();
    SigningKey::from_bytes(&bytes.try_into().expect("bad key length"))
}

fn ledger_path() -> PathBuf {
    key_path().with_file_name("issued-keys.txt")
}

fn ymd(day: i64) -> String {
    let (y, m, d) = date_of_day(day);
    format!("{y:04}-{m:02}-{d:02}")
}

fn parse_date(d: &str) -> i64 {
    let p: Vec<&str> = d.split('-').collect();
    if p.len() != 3 {
        panic!("date must be YYYY-MM-DD");
    }
    day_of_date(
        p[0].parse().expect("year"),
        p[1].parse().expect("month"),
        p[2].parse().expect("day"),
    )
}

/// Issues a key (expires = 0: never), refusing serials already on record, and appends it to the
/// record. Returns the key.
fn issue_recorded(serial: u32, expires: i64, tag: &str) -> String {
    let ledger = ledger_path();
    let existing = std::fs::read_to_string(&ledger).unwrap_or_default();
    let needle = format!("\tserial {serial}\t");
    if existing.lines().any(|l| l.contains(&needle)) {
        eprintln!(
            "Serial {serial} is already in {}; choose another.",
            ledger.display()
        );
        std::process::exit(1);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let issued = day_from_unix(now);
    if expires != 0 && expires <= issued {
        eprintln!("The expiry date must be after today.");
        std::process::exit(1);
    }
    let payload = Payload {
        edition: EDITION_PRO,
        serial,
        issued: issued as u16,
        expires: u16::try_from(expires).expect("expiry out of range"),
    }
    .to_bytes();
    let sig = load().sign(&signed_message(&payload)).to_bytes();
    let key = format_key(&payload, &sig);
    let line = format!(
        "{key}\tserial {serial}\ttag {tag}\tissued {}\texpires {}\n",
        ymd(issued),
        if expires == 0 {
            "never".to_string()
        } else {
            ymd(expires)
        }
    );
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ledger)
        .and_then(|mut f| f.write_all(line.as_bytes()))
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", ledger.display()));
    key
}

fn rust_array(b: &[u8]) -> String {
    let items: Vec<String> = b.iter().map(|x| format!("0x{x:02x}")).collect();
    format!("[{}]", items.join(", "))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("release-init") => {
            let path = release_key_path();
            if path.exists() {
                eprintln!(
                    "{} already exists; refusing to overwrite it.",
                    path.display()
                );
                std::process::exit(1);
            }
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let key = SigningKey::generate(&mut OsRng);
            std::fs::write(&path, hex(&key.to_bytes())).unwrap();
            println!("Release signing key written to {}", path.display());
            println!(
                "Store it as the GitHub secret SHUUSSHY_RELEASE_SIGNING_KEY (hex) and back it up."
            );
            println!("Public key for src-tauri/src/release.rs:");
            println!("{}", rust_array(key.verifying_key().as_bytes()));
        }
        Some("release-pubkey") => {
            println!(
                "{}",
                rust_array(load_from(release_key_path()).verifying_key().as_bytes())
            )
        }
        Some("release-sign") => {
            let (Some(version), Some(channel), Some(date)) =
                (args.get(1), args.get(2), args.get(3))
            else {
                eprintln!("usage: release-sign <version> <channel> <YYYY-MM-DD>");
                std::process::exit(2);
            };
            release::parse_date(date).expect("date must be YYYY-MM-DD");
            let sig = load_from(release_key_path()).sign(&release::message(version, channel, date));
            println!("{}", hex(&sig.to_bytes()));
        }
        Some("init") => {
            let path = key_path();
            if path.exists() {
                eprintln!(
                    "{} already exists; refusing to overwrite it.",
                    path.display()
                );
                std::process::exit(1);
            }
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let key = SigningKey::generate(&mut OsRng);
            std::fs::write(&path, hex(&key.to_bytes())).unwrap();
            println!("Signing key written to {}", path.display());
            println!("Back it up somewhere safe and never commit it.");
            println!("Public key for src-tauri/src/license.rs:");
            println!("{}", rust_array(key.verifying_key().as_bytes()));
        }
        Some("pubkey") => println!("{}", rust_array(load().verifying_key().as_bytes())),
        Some("issue-master") => {
            let serial: u32 = args
                .get(1)
                .and_then(|s| s.parse().ok())
                .expect("usage: issue-master <serial>");
            println!("{}", issue_recorded(serial, 0, "master"));
        }
        Some("issue-pro") => {
            let serial: u32 = args
                .get(1)
                .and_then(|s| s.parse().ok())
                .expect("usage: issue-pro <serial> [YYYY-MM-DD]");
            let expires = match args.get(2) {
                Some(d) => parse_date(d),
                None => {
                    // Two years from today, same calendar day.
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;
                    let (y, m, d) = date_of_day(day_from_unix(now));
                    let d = if m == 2 && d == 29 { 28 } else { d };
                    day_of_date(y + 2, m, d)
                }
            };
            println!("{}", issue_recorded(serial, expires, "pro"));
        }
        Some("issue") => {
            let serial: u32 = args
                .get(1)
                .and_then(|s| s.parse().ok())
                .expect("usage: issue <serial> [YYYY-MM-DD]");
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            let issued = day_from_unix(now) as u16;
            let expires = args.get(2).map_or(0, |d| {
                let p: Vec<&str> = d.split('-').collect();
                let day = day_of_date(
                    p[0].parse().expect("year"),
                    p[1].parse().expect("month"),
                    p[2].parse().expect("day"),
                );
                u16::try_from(day).expect("expiry out of range")
            });
            let payload = Payload {
                edition: EDITION_PRO,
                serial,
                issued,
                expires,
            }
            .to_bytes();
            let sig = load().sign(&signed_message(&payload)).to_bytes();
            println!("{}", format_key(&payload, &sig));
        }
        _ => {
            eprintln!(
                "usage: init | pubkey | issue-pro <serial> [YYYY-MM-DD] | issue-master <serial> | issue <serial> [YYYY-MM-DD]"
            );
            std::process::exit(2);
        }
    }
}
