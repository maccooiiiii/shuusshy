//! Local settings: typed model, validation and SQLite persistence.

pub mod model;
pub mod presets;
pub mod store;

pub use model::Settings;
pub use store::SettingsStore;
