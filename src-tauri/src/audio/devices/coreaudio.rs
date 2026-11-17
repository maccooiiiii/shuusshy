//! macOS device enumeration through cpal's CoreAudio host.
//!
//! CoreAudio lists a device once, with input and/or output channels; Shuusshy splits it into a
//! capture and a render entry with the same ID (the CoreAudio device UID), like Windows' separate
//! endpoints. Apps that play into a device cannot be listed on macOS, so `cable_users` is empty.

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{HostId, InterfaceType};

use super::Snapshot;
use super::model::{AudioDevice, Direction, EndpointState, MixFormat, classify_mac};

fn device_id(d: &cpal::Device) -> Option<String> {
    d.id().ok().map(|id| id.id().to_string())
}

pub fn snapshot() -> Snapshot {
    let host = match cpal::host_from_id(HostId::CoreAudio) {
        Ok(h) => h,
        Err(e) => {
            return Snapshot {
                error: Some(format!("CoreAudio unavailable: {e}")),
                ..Snapshot::default()
            };
        }
    };
    let default_in = host.default_input_device().and_then(|d| device_id(&d));
    let default_out = host.default_output_device().and_then(|d| device_id(&d));
    let devices = match host.devices() {
        Ok(d) => d,
        Err(e) => {
            return Snapshot {
                error: Some(format!("could not list audio devices: {e}")),
                ..Snapshot::default()
            };
        }
    };

    let mut snap = Snapshot::default();
    for device in devices {
        let Some(id) = device_id(&device) else {
            continue;
        };
        let (name, virtual_device) = match device.description() {
            Ok(desc) => (
                desc.name().to_string(),
                matches!(
                    desc.interface_type(),
                    InterfaceType::Aggregate | InterfaceType::Virtual
                ),
            ),
            Err(_) => continue,
        };
        let entry = |direction: Direction, format: MixFormat, is_default: bool| AudioDevice {
            id: id.clone(),
            name: name.clone(),
            description: Some(name.clone()),
            adapter: None,
            enumerator: None,
            direction,
            kind: classify_mac(&name, virtual_device),
            is_default,
            format: Some(format),
            format_error: None,
        };
        if let Ok(c) = device.default_input_config() {
            let format = MixFormat {
                sample_rate: c.sample_rate(),
                channels: c.channels(),
            };
            snap.inputs.push(entry(
                Direction::Capture,
                format,
                default_in.as_deref() == Some(id.as_str()),
            ));
            snap.capture_states
                .insert(id.clone(), EndpointState::Active);
        }
        if let Ok(c) = device.default_output_config() {
            let format = MixFormat {
                sample_rate: c.sample_rate(),
                channels: c.channels(),
            };
            snap.outputs.push(entry(
                Direction::Render,
                format,
                default_out.as_deref() == Some(id.as_str()),
            ));
        }
    }
    snap
}
