//! SQLite-backed settings persistence.
//!
//! Schema is versioned with `PRAGMA user_version`. Each settings section is stored as one JSON
//! row, so adding a field never needs a migration; tables for profiles and release metadata are
//! added by later milestones as new migrations.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use super::model::{AppSettings, AudioSettings, Settings};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("settings database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("settings serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("settings database version {found} is newer than this build supports ({supported})")]
    TooNew { found: i64, supported: i64 },
}

/// Ordered migrations; index + 1 is the resulting `user_version`.
const MIGRATIONS: &[&str] = &[
    // v1: sectioned settings
    "CREATE TABLE settings (
        section     TEXT PRIMARY KEY NOT NULL,
        json        TEXT NOT NULL,
        updated_at  INTEGER NOT NULL
    );",
];

pub struct SettingsStore {
    conn: Connection,
}

impl SettingsStore {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self, StoreError> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// Loads settings. A section with unreadable JSON falls back to defaults rather than
    /// preventing the application from starting; the problem is reported on stderr.
    pub fn load(&self) -> Result<Settings, StoreError> {
        Ok(Settings {
            app: self.load_section::<AppSettings>("app")?,
            audio: self.load_section::<AudioSettings>("audio")?,
        })
    }

    fn load_section<T: serde::de::DeserializeOwned + Default>(
        &self,
        section: &str,
    ) -> Result<T, StoreError> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM settings WHERE section = ?1",
                params![section],
                |r| r.get(0),
            )
            .optional()?;
        Ok(match raw {
            None => T::default(),
            Some(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
                eprintln!(
                    "shuusshy: settings section '{section}' unreadable ({e}); using defaults"
                );
                T::default()
            }),
        })
    }

    pub fn save(&mut self, settings: &Settings) -> Result<(), StoreError> {
        let now = unix_now();
        let tx = self.conn.transaction()?;
        for (section, json) in [
            ("app", serde_json::to_string(&settings.app)?),
            ("audio", serde_json::to_string(&settings.audio)?),
        ] {
            tx.execute(
                "INSERT INTO settings (section, json, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(section) DO UPDATE SET json = excluded.json, updated_at = excluded.updated_at",
                params![section, json, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn reset(&mut self) -> Result<Settings, StoreError> {
        self.conn.execute("DELETE FROM settings", [])?;
        Ok(Settings::default())
    }
}

fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let supported = MIGRATIONS.len() as i64;
    if current > supported {
        return Err(StoreError::TooNew {
            found: current,
            supported,
        });
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::model::Engine;

    #[test]
    fn empty_database_loads_defaults() {
        let store = SettingsStore::open_in_memory().unwrap();
        assert_eq!(store.load().unwrap(), Settings::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let mut store = SettingsStore::open_in_memory().unwrap();
        let mut s = Settings::default();
        s.app.always_on_top = false;
        s.audio.engine = Engine::DeepFilterNet;
        s.audio.input_device_id = Some("mic-1".into());
        store.save(&s).unwrap();
        assert_eq!(store.load().unwrap(), s);
        // Saving twice must upsert, not fail on the primary key.
        store.save(&s).unwrap();
        assert_eq!(store.load().unwrap(), s);
    }

    #[test]
    fn reset_restores_defaults() {
        let mut store = SettingsStore::open_in_memory().unwrap();
        let mut s = Settings::default();
        s.app.always_on_top = false;
        store.save(&s).unwrap();
        assert_eq!(store.reset().unwrap(), Settings::default());
        assert_eq!(store.load().unwrap(), Settings::default());
    }

    #[test]
    fn corrupt_section_falls_back_to_defaults() {
        let store = SettingsStore::open_in_memory().unwrap();
        store
            .conn
            .execute(
                "INSERT INTO settings (section, json, updated_at) VALUES ('app', 'not json', 0)",
                [],
            )
            .unwrap();
        assert_eq!(store.load().unwrap().app, AppSettings::default());
    }

    #[test]
    fn migration_sets_user_version_and_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();
        let v: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, MIGRATIONS.len() as i64);
    }

    #[test]
    fn refuses_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();
        assert!(matches!(
            migrate(&mut conn),
            Err(StoreError::TooNew { found: 999, .. })
        ));
    }
}
