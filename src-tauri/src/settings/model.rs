//! Typed settings model and validation.
//!
//! Every field has a default so that settings written by an older version (missing fields)
//! still load. Validation runs on every value that crosses the IPC boundary from the frontend.

use serde::{Deserialize, Serialize};

/// Bounds used by validation. Shared with the frontend via documentation, enforced here.
pub const GAIN_DB_MIN: f32 = -24.0;
pub const GAIN_DB_MAX: f32 = 24.0;
pub const GATE_THRESHOLD_DB_MIN: f32 = -90.0;
pub const GATE_THRESHOLD_DB_MAX: f32 = 0.0;
pub const ISOLATION_MARGIN_DB_MIN: f32 = 3.0;
pub const ISOLATION_MARGIN_DB_MAX: f32 = 24.0;
pub const MAX_DEVICE_ID_LEN: usize = 512;
pub const GLASS_TINT_MIN: f32 = 0.35;
pub const GLASS_TINT_MAX: f32 = 0.95;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub app: AppSettings,
    pub audio: AudioSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Keep the floating banner above other windows.
    pub always_on_top: bool,
    /// Show the compact banner (status, microphone, on/off, test) instead of the expanded one.
    pub compact: bool,
    /// Closing the banner hides it to the system tray and keeps processing running. When off,
    /// closing the banner exits Shuusshy.
    pub keep_running_in_tray: bool,
    /// Shuusshy Pro license key. Set only through `activate_license`, which verifies it.
    pub license_key: Option<String>,
    /// The first-run "activate Pro" card was dismissed.
    pub activation_prompt_dismissed: bool,
    /// See-through glass window (the desktop shows blurred behind the banner).
    pub glass: bool,
    /// How much of the dark tint covers the blurred desktop (0.35 = most see-through).
    pub glass_tint: f32,
    pub accent: Accent,
}

/// Accent colour for highlights, the emblem and the tray icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Accent {
    #[default]
    Cyan,
    Violet,
    Mint,
    Amber,
    Rose,
    Ice,
    /// The Windows accent colour (Settings > Personalization > Colors).
    System,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            always_on_top: true,
            compact: false,
            keep_running_in_tray: true,
            license_key: None,
            activation_prompt_dismissed: false,
            glass: true,
            glass_tint: 0.6,
            accent: Accent::Cyan,
        }
    }
}

/// Noise suppression engine choice (RNNoise or DeepFilterNet).
/// Selectable since Milestone 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    #[default]
    Rnnoise,
    DeepFilterNet,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NoiseGate {
    pub enabled: bool,
    pub threshold_db: f32,
}

impl Default for NoiseGate {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_db: -50.0,
        }
    }
}

/// Voice isolation: an adaptive gate that mutes speech quieter than the user's own voice while
/// the user pauses (see `audio/chain.rs`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VoiceIsolation {
    pub enabled: bool,
    /// Threshold distance below the tracked voice level, in dB. Smaller = stricter.
    pub margin_db: f32,
}

impl Default for VoiceIsolation {
    fn default() -> Self {
        Self {
            enabled: false,
            margin_db: 6.0,
        }
    }
}

/// Audio configuration. Milestone 3 uses the microphone selection and `routing_enabled`; the
/// engine, suppression, gain and gate fields take effect in Milestones 4-6.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AudioSettings {
    /// WASAPI endpoint ID of the selected physical microphone. Set only via `select_input_device`.
    pub input_device_id: Option<String>,
    /// Last known friendly name of that microphone, shown while it is disconnected.
    pub input_device_name: Option<String>,
    pub virtual_device_id: Option<String>,
    /// Send the microphone to the virtual microphone (VB-CABLE) while Shuusshy runs.
    pub routing_enabled: bool,
    pub engine: Engine,
    pub suppression_enabled: bool,
    /// 0.0 = no suppression, 1.0 = maximum supported by the active engine.
    pub suppression_strength: f32,
    pub input_gain_db: f32,
    pub output_gain_db: f32,
    pub noise_gate: NoiseGate,
    pub voice_isolation: VoiceIsolation,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            input_device_id: None,
            input_device_name: None,
            virtual_device_id: None,
            routing_enabled: true,
            engine: Engine::default(),
            suppression_enabled: false,
            suppression_strength: 0.8,
            input_gain_db: 0.0,
            output_gain_db: 0.0,
            noise_gate: NoiseGate::default(),
            voice_isolation: VoiceIsolation::default(),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ValidationError {
    #[error("{field} must be a finite number between {min} and {max}")]
    OutOfRange {
        field: &'static str,
        min: f32,
        max: f32,
    },
    #[error("{field} is invalid (empty, too long, or contains control characters)")]
    BadDeviceId { field: &'static str },
}

fn check_range(field: &'static str, v: f32, min: f32, max: f32) -> Result<(), ValidationError> {
    if v.is_finite() && (min..=max).contains(&v) {
        Ok(())
    } else {
        Err(ValidationError::OutOfRange { field, min, max })
    }
}

fn check_device_id(field: &'static str, id: &Option<String>) -> Result<(), ValidationError> {
    match id {
        None => Ok(()),
        Some(s)
            if !s.is_empty()
                && s.len() <= MAX_DEVICE_ID_LEN
                && !s.chars().any(char::is_control) =>
        {
            Ok(())
        }
        Some(_) => Err(ValidationError::BadDeviceId { field }),
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), ValidationError> {
        let a = &self.audio;
        check_device_id("app.licenseKey", &self.app.license_key)?;
        check_range(
            "app.glassTint",
            self.app.glass_tint,
            GLASS_TINT_MIN,
            GLASS_TINT_MAX,
        )?;
        check_device_id("audio.inputDeviceId", &a.input_device_id)?;
        check_device_id("audio.inputDeviceName", &a.input_device_name)?;
        check_device_id("audio.virtualDeviceId", &a.virtual_device_id)?;
        check_range(
            "audio.suppressionStrength",
            a.suppression_strength,
            0.0,
            1.0,
        )?;
        check_range(
            "audio.inputGainDb",
            a.input_gain_db,
            GAIN_DB_MIN,
            GAIN_DB_MAX,
        )?;
        check_range(
            "audio.outputGainDb",
            a.output_gain_db,
            GAIN_DB_MIN,
            GAIN_DB_MAX,
        )?;
        check_range(
            "audio.noiseGate.thresholdDb",
            a.noise_gate.threshold_db,
            GATE_THRESHOLD_DB_MIN,
            GATE_THRESHOLD_DB_MAX,
        )?;
        check_range(
            "audio.voiceIsolation.marginDb",
            a.voice_isolation.margin_db,
            ISOLATION_MARGIN_DB_MIN,
            ISOLATION_MARGIN_DB_MAX,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert_eq!(Settings::default().validate(), Ok(()));
    }

    #[test]
    fn rejects_out_of_range_and_non_finite() {
        let mut s = Settings::default();
        s.audio.suppression_strength = 1.5;
        assert!(s.validate().is_err());
        s.audio.suppression_strength = f32::NAN;
        assert!(s.validate().is_err());
        s.audio.suppression_strength = 0.5;
        s.audio.input_gain_db = 30.0;
        assert!(s.validate().is_err());
        s.audio.input_gain_db = 0.0;
        s.audio.noise_gate.threshold_db = 5.0;
        assert!(s.validate().is_err());
    }

    #[test]
    fn rejects_bad_device_ids() {
        let mut s = Settings::default();
        s.audio.input_device_id = Some(String::new());
        assert!(s.validate().is_err());
        s.audio.input_device_id = Some("a\u{0}b".into());
        assert!(s.validate().is_err());
        s.audio.input_device_id = Some("x".repeat(MAX_DEVICE_ID_LEN + 1));
        assert!(s.validate().is_err());
        s.audio.input_device_id = Some("{0.0.1.00000000}.{abc}".into());
        assert_eq!(s.validate(), Ok(()));
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"audio":{"engine":"deepFilterNet"}}"#).unwrap();
        assert_eq!(s.audio.engine, Engine::DeepFilterNet);
        assert_eq!(s.audio.suppression_strength, 0.8);
        assert!(s.app.always_on_top);
        // Settings saved before routing existed must default to routing on.
        assert!(s.audio.routing_enabled);
        // Settings saved before Milestone 8 get the expanded banner and close-to-tray.
        let old: Settings = serde_json::from_str(r#"{"app":{"alwaysOnTop":false}}"#).unwrap();
        assert!(!old.app.always_on_top);
        assert!(!old.app.compact);
        assert!(old.app.keep_running_in_tray);
    }

    #[test]
    fn json_uses_camel_case() {
        let json = serde_json::to_string(&Settings::default()).unwrap();
        assert!(json.contains("alwaysOnTop"));
        assert!(json.contains("suppressionStrength"));
        assert!(json.contains("\"rnnoise\""));
    }
}
