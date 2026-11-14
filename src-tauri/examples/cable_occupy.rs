//! Test helper: plays digital silence (inaudible) into a VB-CABLE playback endpoint for a while,
//! reproducing an app whose speaker/output is set to the virtual cable. Used to verify that
//! Shuusshy detects and names such apps.
//!
//! ```text
//! cargo run --release --example cable_occupy -- [16ch|input] [seconds=15]
//! ```

use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{DeviceId, HostId};
use shuusshy_lib::audio::devices::{enumerate_once, model::DeviceKind};

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "16ch".into());
    let secs: u64 = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(15);
    let snap = enumerate_once().expect("enumerate");
    let target = snap
        .outputs
        .iter()
        .filter(|d| d.kind == DeviceKind::VbCable)
        .find(|d| {
            let label = d.description.as_deref().unwrap_or(&d.name);
            if which == "16ch" {
                label.contains("16ch")
            } else {
                label.eq_ignore_ascii_case("CABLE Input")
            }
        })
        .expect("VB-CABLE playback endpoint not found");
    let host = cpal::host_from_id(HostId::Wasapi).unwrap();
    let dev = host
        .device_by_id(&DeviceId::new(HostId::Wasapi, &target.id))
        .unwrap();
    let cfg = dev.default_output_config().unwrap();
    let stream = dev
        .build_output_stream(
            cfg.config(),
            |data: &mut [f32], _| data.fill(0.0),
            |e| eprintln!("error: {e}"),
            None,
        )
        .expect("open stream");
    stream.play().unwrap();
    println!(
        "playing silence into {} for {secs} s (pid {})",
        target.name,
        std::process::id()
    );
    std::thread::sleep(Duration::from_secs(secs));
}
