//! Audio presets (spec Part 3.4). Each preset is a concrete engine configuration; nothing is
//! advertised that the engine cannot do. Presets set the engine, strength and noise gate and
//! switch noise cancellation on. They leave input/output gain alone (those match the user's
//! microphone, not the noise situation). Any other combination is "Custom".

use serde::Serialize;

use super::model::{AudioSettings, Engine};

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub engine: Engine,
    pub strength: f32,
    pub gate_enabled: bool,
    pub gate_threshold_db: f32,
    pub isolation_enabled: bool,
    pub isolation_margin_db: f32,
}

/// The selectable presets. Strength meanings: RNNoise = wet/dry mix; DeepFilterNet = attenuation
/// limit (80 % ≈ 36 dB, 50 % ≈ 22 dB, 100 % = unlimited).
pub const PRESETS: [PresetInfo; 4] = [
    PresetInfo {
        id: "balanced",
        name: "Balanced",
        description: "DeepFilterNet at 80 % (up to ~36 dB less noise), natural voice",
        engine: Engine::DeepFilterNet,
        strength: 0.8,
        gate_enabled: false,
        gate_threshold_db: -50.0,
        isolation_enabled: false,
        isolation_margin_db: 6.0,
    },
    PresetInfo {
        id: "strong",
        name: "Strong Noise Reduction",
        description: "DeepFilterNet at full strength plus voice isolation: mutes quieter background speech while you pause",
        engine: Engine::DeepFilterNet,
        strength: 1.0,
        gate_enabled: false,
        gate_threshold_db: -50.0,
        isolation_enabled: true,
        isolation_margin_db: 6.0,
    },
    PresetInfo {
        id: "speech",
        name: "Speech Priority",
        description: "DeepFilterNet limited to ~22 dB, so the voice stays most natural",
        engine: Engine::DeepFilterNet,
        strength: 0.5,
        gate_enabled: false,
        gate_threshold_db: -50.0,
        isolation_enabled: false,
        isolation_margin_db: 6.0,
    },
    PresetInfo {
        id: "lowCpu",
        name: "Low CPU Usage",
        description: "RNNoise at 80 %: about a quarter of DeepFilterNet's CPU, 10 ms delay",
        engine: Engine::Rnnoise,
        strength: 0.8,
        gate_enabled: false,
        gate_threshold_db: -50.0,
        isolation_enabled: false,
        isolation_margin_db: 6.0,
    },
];

pub const CUSTOM: &str = "custom";

/// Applies preset `id` to `audio`.
/// True if the preset uses a Pro feature (DeepFilterNet or voice isolation).
pub fn needs_pro(p: &PresetInfo) -> bool {
    p.engine == Engine::DeepFilterNet || p.isolation_enabled
}

pub fn apply(audio: &mut AudioSettings, id: &str) -> Result<(), String> {
    let p = PRESETS
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("unknown preset \"{id}\""))?;
    audio.engine = p.engine;
    audio.suppression_strength = p.strength;
    audio.noise_gate.enabled = p.gate_enabled;
    audio.noise_gate.threshold_db = p.gate_threshold_db;
    audio.voice_isolation.enabled = p.isolation_enabled;
    audio.voice_isolation.margin_db = p.isolation_margin_db;
    audio.suppression_enabled = true;
    Ok(())
}

/// The preset whose configuration `audio` currently has, or `"custom"`.
pub fn matching(audio: &AudioSettings) -> &'static str {
    PRESETS
        .iter()
        .find(|p| {
            p.engine == audio.engine
                && (p.strength - audio.suppression_strength).abs() < 0.005
                && p.gate_enabled == audio.noise_gate.enabled
                && (!p.gate_enabled
                    || (p.gate_threshold_db - audio.noise_gate.threshold_db).abs() < 0.05)
                && p.isolation_enabled == audio.voice_isolation.enabled
                && (!p.isolation_enabled
                    || (p.isolation_margin_db - audio.voice_isolation.margin_db).abs() < 0.05)
        })
        .map_or(CUSTOM, |p| p.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn every_preset_is_a_valid_configuration_and_round_trips() {
        for p in PRESETS {
            let mut s = Settings::default();
            s.audio.input_gain_db = 6.0;
            apply(&mut s.audio, p.id).unwrap();
            assert_eq!(s.validate(), Ok(()), "{}", p.id);
            assert_eq!(matching(&s.audio), p.id);
            assert!(s.audio.suppression_enabled);
            assert_eq!(s.audio.input_gain_db, 6.0, "presets must not touch gains");
        }
    }

    #[test]
    fn presets_are_distinct() {
        for (i, a) in PRESETS.iter().enumerate() {
            for b in &PRESETS[i + 1..] {
                let mut s = AudioSettings::default();
                apply(&mut s, a.id).unwrap();
                assert_ne!(matching(&s), b.id);
            }
        }
    }

    #[test]
    fn manual_changes_become_custom() {
        let mut s = AudioSettings::default();
        apply(&mut s, "balanced").unwrap();
        s.suppression_strength = 0.65;
        assert_eq!(matching(&s), CUSTOM);
        apply(&mut s, "strong").unwrap();
        s.voice_isolation.margin_db = 9.0;
        assert_eq!(matching(&s), CUSTOM);
        assert!(apply(&mut s, "nonsense").is_err());
    }
}
