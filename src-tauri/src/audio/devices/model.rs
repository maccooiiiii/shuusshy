//! Platform-neutral device model: classification, VB-CABLE identification and microphone status.
//! Everything here is pure so it can be unit-tested without audio hardware.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    /// Recording endpoint (microphones, "CABLE Output").
    Capture,
    /// Playback endpoint (speakers, "CABLE Input").
    Render,
}

/// What kind of endpoint this is, as far as Shuusshy is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceKind {
    /// Hardware microphone / speaker (built-in, USB, Bluetooth, ...).
    Physical,
    /// A VB-Audio Virtual Cable endpoint.
    VbCable,
    /// Another software / virtual device (e.g. other noise-suppression apps, mixers).
    OtherVirtual,
}

/// Windows endpoint state (`DEVICE_STATE_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EndpointState {
    Active,
    Disabled,
    NotPresent,
    Unplugged,
    Unknown,
}

/// Shared-mode mix format reported by the audio engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    /// Stable WASAPI endpoint ID, e.g. `{0.0.1.00000000}.{guid}`.
    pub id: String,
    /// Full friendly name, e.g. `Microphone (Realtek High Definition Audio)`.
    pub name: String,
    /// Endpoint description, e.g. `Microphone` or `CABLE Output`.
    pub description: Option<String>,
    /// Adapter / driver name, e.g. `Realtek High Definition Audio`.
    pub adapter: Option<String>,
    /// PnP enumerator, e.g. `HDAUDIO`, `USB`, `BTHENUM`, `ROOT`.
    pub enumerator: Option<String>,
    pub direction: Direction,
    pub kind: DeviceKind,
    pub is_default: bool,
    /// `None` with `format_error` set when the endpoint could not be opened.
    pub format: Option<MixFormat>,
    pub format_error: Option<String>,
}

/// Raw properties read from an endpoint, before classification.
#[derive(Debug, Clone, Default)]
pub struct EndpointProps<'a> {
    pub adapter: Option<&'a str>,
    pub enumerator: Option<&'a str>,
}

/// Software device drivers that are not virtual cables but should never be used as the "physical"
/// microphone (they would double-process audio or create loops). Matched against the adapter name.
const KNOWN_VIRTUAL_ADAPTERS: &[&str] = &[
    "krisp",
    "voicemeeter",
    "nvidia broadcast",
    "steam streaming",
    "virtual audio",
];

pub fn classify(p: &EndpointProps) -> DeviceKind {
    let adapter = p.adapter.unwrap_or("").to_ascii_lowercase();
    if adapter.contains("vb-audio") {
        return DeviceKind::VbCable;
    }
    // ROOT-enumerated and software-device (SW/SWD) endpoints are not backed by hardware buses.
    let enumerator = p.enumerator.unwrap_or("").to_ascii_uppercase();
    if matches!(enumerator.as_str(), "ROOT" | "SW" | "SWD")
        || KNOWN_VIRTUAL_ADAPTERS.iter().any(|v| adapter.contains(v))
    {
        return DeviceKind::OtherVirtual;
    }
    DeviceKind::Physical
}

/// Virtual or software devices on macOS that must never be used as the "physical" microphone.
/// Matched against the CoreAudio device name.
const KNOWN_VIRTUAL_MAC_NAMES: &[&str] = &[
    "blackhole",
    "vb-cable",
    "zoomaudiodevice",
    "microsoft teams audio",
    "loopback",
    "soundflower",
    "multi-output",
    "aggregate",
    "krisp",
    "ecamm",
    "obs",
    "background music",
];

/// macOS classification. CoreAudio exposes a device under one name for both directions;
/// [`BLACKHOLE_LABEL`] is the supported virtual cable, `virtual_device` covers aggregate devices.
pub fn classify_mac(name: &str, virtual_device: bool) -> DeviceKind {
    let n = name.to_ascii_lowercase();
    if name.eq_ignore_ascii_case(BLACKHOLE_LABEL) {
        return DeviceKind::VbCable;
    }
    if virtual_device || KNOWN_VIRTUAL_MAC_NAMES.iter().any(|v| n.contains(v)) {
        return DeviceKind::OtherVirtual;
    }
    DeviceKind::Physical
}

/// The virtual cable Shuusshy uses on macOS (BlackHole, 2-channel edition). One CoreAudio device
/// with both sides: Shuusshy plays into its output, other apps record from its input.
pub const BLACKHOLE_LABEL: &str = "BlackHole 2ch";

/// The virtual cable driver's name, for messages.
#[cfg(windows)]
pub const CABLE_NAME: &str = "VB-CABLE";
#[cfg(not(windows))]
pub const CABLE_NAME: &str = "BlackHole";

/// VB-CABLE's own name for its recording endpoint.
pub const CABLE_CAPTURE_LABEL: &str = "CABLE Output";
/// The name Shuusshy can give that endpoint so apps list "Shuusshy Microphone".
pub const SHUUSSHY_MIC_LABEL: &str = "Shuusshy Microphone";

/// True for the standard 2-channel VB-CABLE endpoints (`CABLE Input` / `CABLE Output`),
/// excluding the `CABLE In 16ch` variant and other VB-Audio products (CABLE-A, Hi-Fi Cable...).
pub fn is_primary_cable_endpoint(d: &AudioDevice) -> bool {
    if d.kind != DeviceKind::VbCable {
        return false;
    }
    let label = d.description.as_deref().unwrap_or(&d.name);
    if label.eq_ignore_ascii_case(BLACKHOLE_LABEL) {
        return true;
    }
    match d.direction {
        Direction::Render => label.eq_ignore_ascii_case("CABLE Input"),
        Direction::Capture => {
            label.eq_ignore_ascii_case(CABLE_CAPTURE_LABEL)
                || label.eq_ignore_ascii_case(SHUUSSHY_MIC_LABEL)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum CableStatus {
    /// Both endpoints present: Shuusshy writes to `render`, apps record from `capture`.
    Ready {
        render: AudioDevice,
        capture: AudioDevice,
    },
    /// Only one side is present/enabled.
    Incomplete {
        render: Option<AudioDevice>,
        capture: Option<AudioDevice>,
    },
    NotInstalled,
}

pub fn cable_status(inputs: &[AudioDevice], outputs: &[AudioDevice]) -> CableStatus {
    let capture = inputs
        .iter()
        .find(|d| is_primary_cable_endpoint(d))
        .cloned();
    // On macOS both sides are one device: prefer the playback side of the same device.
    let render = capture
        .as_ref()
        .and_then(|c| {
            outputs
                .iter()
                .find(|d| d.id == c.id && is_primary_cable_endpoint(d))
        })
        .or_else(|| outputs.iter().find(|d| is_primary_cable_endpoint(d)))
        .cloned();
    match (render, capture) {
        (Some(render), Some(capture)) => CableStatus::Ready { render, capture },
        (None, None) => CableStatus::NotInstalled,
        (render, capture) => CableStatus::Incomplete { render, capture },
    }
}

/// Windows audio host processes that may hold sessions on any endpoint (e.g. "Listen to this
/// device"); they are not user apps the user can reconfigure.
const SYSTEM_AUDIO_HOSTS: &[&str] = &["audiodg", "svchost", "explorer"];

/// Cleans a list of process names found playing into the virtual cable: removes system audio
/// hosts, de-duplicates (case-insensitively) and sorts.
pub fn conflicting_apps(names: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in names {
        if SYSTEM_AUDIO_HOSTS.iter().any(|h| n.eq_ignore_ascii_case(h)) {
            continue;
        }
        if !out.iter().any(|o| o.eq_ignore_ascii_case(&n)) {
            out.push(n);
        }
    }
    out.sort_by_key(|n| n.to_ascii_lowercase());
    out
}

/// Status of the user's selected physical microphone.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum MicStatus {
    NotSelected,
    Available {
        device: AudioDevice,
    },
    /// Present but could not be opened (format query failed).
    Error {
        device: AudioDevice,
        message: String,
    },
    /// Selected device is not currently active. `name` is the last known name.
    Disconnected {
        id: String,
        name: Option<String>,
        reason: EndpointState,
    },
}

pub fn mic_status(
    selected_id: Option<&str>,
    last_known_name: Option<&str>,
    inputs: &[AudioDevice],
    state_of: impl Fn(&str) -> EndpointState,
) -> MicStatus {
    let Some(id) = selected_id else {
        return MicStatus::NotSelected;
    };
    match inputs.iter().find(|d| d.id == id) {
        Some(d) => match &d.format_error {
            Some(msg) => MicStatus::Error {
                device: d.clone(),
                message: msg.clone(),
            },
            None => MicStatus::Available { device: d.clone() },
        },
        None => MicStatus::Disconnected {
            id: id.to_owned(),
            name: last_known_name.map(str::to_owned),
            reason: match state_of(id) {
                // Listed as active but not in our snapshot: treat as transient.
                EndpointState::Active => EndpointState::Unknown,
                s => s,
            },
        },
    }
}

/// Microphone to pick automatically on first run: the system default if it is physical,
/// otherwise the first physical input. Never a virtual device.
pub fn auto_select(inputs: &[AudioDevice]) -> Option<&AudioDevice> {
    let physical = || inputs.iter().filter(|d| d.kind == DeviceKind::Physical);
    physical()
        .find(|d| d.is_default)
        .or_else(|| physical().next())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn dev(
        id: &str,
        name: &str,
        desc: &str,
        adapter: &str,
        enumerator: &str,
        direction: Direction,
    ) -> AudioDevice {
        let kind = classify(&EndpointProps {
            adapter: Some(adapter),
            enumerator: Some(enumerator),
        });
        AudioDevice {
            id: id.into(),
            name: name.into(),
            description: Some(desc.into()),
            adapter: Some(adapter.into()),
            enumerator: Some(enumerator.into()),
            direction,
            kind,
            is_default: false,
            format: Some(MixFormat {
                sample_rate: 48_000,
                channels: 2,
            }),
            format_error: None,
        }
    }

    fn mac(id: &str, name: &str, direction: Direction, virtual_device: bool) -> AudioDevice {
        AudioDevice {
            id: id.into(),
            name: name.into(),
            description: Some(name.into()),
            adapter: None,
            enumerator: None,
            direction,
            kind: classify_mac(name, virtual_device),
            is_default: false,
            format: Some(MixFormat {
                sample_rate: 48_000,
                channels: 2,
            }),
            format_error: None,
        }
    }

    #[test]
    fn mac_classification_and_blackhole_pairing() {
        assert_eq!(
            classify_mac("MacBook Pro Microphone", false),
            DeviceKind::Physical
        );
        assert_eq!(classify_mac("BlackHole 2ch", false), DeviceKind::VbCable);
        assert_eq!(
            classify_mac("BlackHole 16ch", false),
            DeviceKind::OtherVirtual
        );
        assert_eq!(
            classify_mac("ZoomAudioDevice", false),
            DeviceKind::OtherVirtual
        );
        assert_eq!(classify_mac("My Combo", true), DeviceKind::OtherVirtual);

        let ins = [
            mac(
                "BuiltInMic",
                "MacBook Pro Microphone",
                Direction::Capture,
                false,
            ),
            mac("BH16", "BlackHole 16ch", Direction::Capture, false),
            mac("BH2", "BlackHole 2ch", Direction::Capture, false),
        ];
        let outs = [
            mac("BH16", "BlackHole 16ch", Direction::Render, false),
            mac("BH2", "BlackHole 2ch", Direction::Render, false),
        ];
        match cable_status(&ins, &outs) {
            CableStatus::Ready { render, capture } => {
                assert_eq!(render.id, "BH2");
                assert_eq!(capture.id, "BH2");
            }
            other => panic!("expected ready, got {other:?}"),
        }
        assert_eq!(auto_select(&ins).map(|d| d.id.as_str()), Some("BuiltInMic"));
    }

    // Property values below are copied from the development machine's registry (Milestone 2).
    fn realtek_mic() -> AudioDevice {
        dev(
            "{0.0.1.00000000}.{bd1f7f5b}",
            "Microphone (Realtek High Definition Audio)",
            "Microphone",
            "Realtek High Definition Audio",
            "HDAUDIO",
            Direction::Capture,
        )
    }
    fn cable_output() -> AudioDevice {
        dev(
            "{0.0.1.00000000}.{aa6810c3}",
            "CABLE Output (VB-Audio Virtual Cable)",
            "CABLE Output",
            "VB-Audio Virtual Cable",
            "ROOT",
            Direction::Capture,
        )
    }
    fn cable_input() -> AudioDevice {
        dev(
            "{0.0.0.00000000}.{c1}",
            "CABLE Input (VB-Audio Virtual Cable)",
            "CABLE Input",
            "VB-Audio Virtual Cable",
            "ROOT",
            Direction::Render,
        )
    }
    fn cable_16ch() -> AudioDevice {
        dev(
            "{0.0.0.00000000}.{c16}",
            "CABLE In 16ch (VB-Audio Virtual Cable)",
            "CABLE In 16ch",
            "VB-Audio Virtual Cable",
            "ROOT",
            Direction::Render,
        )
    }

    #[test]
    fn renamed_virtual_mic_is_still_vb_cable() {
        let renamed = dev(
            "{0.0.1.00000000}.{aa6810c3}",
            "Shuusshy Microphone (VB-Audio Virtual Cable)",
            "Shuusshy Microphone",
            "VB-Audio Virtual Cable",
            "ROOT",
            Direction::Capture,
        );
        assert!(is_primary_cable_endpoint(&renamed));
        assert!(matches!(
            cable_status(&[renamed], &[cable_input()]),
            CableStatus::Ready { .. }
        ));
        // Any other name is not taken for VB-CABLE's primary endpoint.
        let other = dev(
            "x",
            "Foo (VB-Audio Virtual Cable)",
            "Foo",
            "VB-Audio Virtual Cable",
            "ROOT",
            Direction::Capture,
        );
        assert!(!is_primary_cable_endpoint(&other));
    }

    #[test]
    fn classifies_real_devices() {
        assert_eq!(realtek_mic().kind, DeviceKind::Physical);
        assert_eq!(cable_output().kind, DeviceKind::VbCable);
        let krisp = classify(&EndpointProps {
            adapter: Some("Krisp Audio"),
            enumerator: Some("ROOT"),
        });
        assert_eq!(krisp, DeviceKind::OtherVirtual);
        let usb = classify(&EndpointProps {
            adapter: Some("USB Audio Device"),
            enumerator: Some("USB"),
        });
        assert_eq!(usb, DeviceKind::Physical);
        let bt = classify(&EndpointProps {
            adapter: Some("WH-1000XM4 Hands-Free AG Audio"),
            enumerator: Some("BTHHFENUM"),
        });
        assert_eq!(bt, DeviceKind::Physical);
    }

    #[test]
    fn identifies_primary_cable_endpoints_only() {
        assert!(is_primary_cable_endpoint(&cable_output()));
        assert!(is_primary_cable_endpoint(&cable_input()));
        assert!(!is_primary_cable_endpoint(&cable_16ch()));
        assert!(!is_primary_cable_endpoint(&realtek_mic()));
    }

    #[test]
    fn cable_status_variants() {
        let ins = [realtek_mic(), cable_output()];
        let outs = [cable_16ch(), cable_input()];
        assert!(matches!(
            cable_status(&ins, &outs),
            CableStatus::Ready { ref render, .. } if render.description.as_deref() == Some("CABLE Input")
        ));
        assert!(matches!(
            cable_status(&ins, &[cable_16ch()]),
            CableStatus::Incomplete {
                render: None,
                capture: Some(_)
            }
        ));
        assert_eq!(
            cable_status(&[realtek_mic()], &[]),
            CableStatus::NotInstalled
        );
    }

    #[test]
    fn mic_status_cases() {
        let inputs = [realtek_mic(), cable_output()];
        let gone = |_: &str| EndpointState::Unplugged;
        assert_eq!(
            mic_status(None, None, &inputs, gone),
            MicStatus::NotSelected
        );
        assert!(matches!(
            mic_status(Some("{0.0.1.00000000}.{bd1f7f5b}"), None, &inputs, gone),
            MicStatus::Available { .. }
        ));
        assert_eq!(
            mic_status(Some("usb-mic"), Some("USB Mic"), &inputs, gone),
            MicStatus::Disconnected {
                id: "usb-mic".into(),
                name: Some("USB Mic".into()),
                reason: EndpointState::Unplugged
            }
        );
        let mut broken = realtek_mic();
        broken.format_error = Some("AUDCLNT_E_DEVICE_INVALIDATED".into());
        assert!(matches!(
            mic_status(Some(&broken.id.clone()), None, &[broken], gone),
            MicStatus::Error { .. }
        ));
    }

    #[test]
    fn auto_select_never_picks_virtual() {
        let mut cable = cable_output();
        cable.is_default = true;
        let mic = realtek_mic();
        assert_eq!(
            auto_select(&[cable.clone(), mic.clone()]).map(|d| &d.id),
            Some(&mic.id)
        );
        assert_eq!(auto_select(&[cable]), None);
    }

    #[test]
    fn conflicting_apps_are_cleaned_up() {
        let names = [
            "Discord", "svchost", "discord", "Zoom", "audiodg", "Discord",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(conflicting_apps(names), vec!["Discord", "Zoom"]);
        assert!(conflicting_apps(vec![]).is_empty());
    }

    #[test]
    fn auto_select_prefers_default_physical() {
        let a = dev("a", "Mic A", "Mic", "USB Audio", "USB", Direction::Capture);
        let mut b = dev(
            "b",
            "Mic B",
            "Mic",
            "Realtek",
            "HDAUDIO",
            Direction::Capture,
        );
        b.is_default = true;
        assert_eq!(auto_select(&[a, b]).map(|d| d.id.as_str()), Some("b"));
    }
}
