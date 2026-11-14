//! Appearance: see-through glass window, accent colours, and accent-tinted tray frames.

use tauri::WebviewWindow;
use tauri::window::{Color, Effect, EffectsBuilder};

use crate::settings::model::{Accent, AppSettings};

/// The accent the emblem and tray frames were rendered in (Blender cyan).
pub const RENDER_ACCENT: [u8; 3] = [0x39, 0xd5, 0xff];

/// Fixed accent colours (sRGB). `System` reads the Windows accent.
pub fn accent_rgb(accent: Accent) -> [u8; 3] {
    match accent {
        Accent::Cyan => RENDER_ACCENT,
        Accent::Violet => [0x9b, 0x8c, 0xff],
        Accent::Mint => [0x4d, 0xe3, 0xb0],
        Accent::Amber => [0xff, 0xb5, 0x47],
        Accent::Rose => [0xff, 0x6b, 0x9d],
        Accent::Ice => [0xc7, 0xd7, 0xea],
        Accent::System => system_accent().map(readable).unwrap_or(RENDER_ACCENT),
    }
}

/// The Windows accent / colorization colour.
pub fn system_accent() -> Option<[u8; 3]> {
    use windows::Win32::Graphics::Dwm::DwmGetColorizationColor;
    let mut argb = 0u32;
    let mut opaque = windows_core::BOOL(0);
    // SAFETY: both out-pointers are valid for the call.
    unsafe { DwmGetColorizationColor(&mut argb, &mut opaque) }.ok()?;
    Some([(argb >> 16) as u8, (argb >> 8) as u8, argb as u8])
}

/// Lifts dark accents so they stay readable on the dark glass (lightness >= 0.62).
pub fn readable(rgb: [u8; 3]) -> [u8; 3] {
    let (h, s, l) = rgb_to_hsl(rgb);
    hsl_to_rgb(h, s, l.max(0.62))
}

pub fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// Windows build number (e.g. 19045 = Windows 10 22H2, 22000+ = Windows 11).
pub fn windows_build() -> u32 {
    use windows::Wdk::System::SystemServices::RtlGetVersion;
    use windows::Win32::System::SystemInformation::OSVERSIONINFOW;
    let mut v = OSVERSIONINFOW {
        dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // SAFETY: v is a properly sized OSVERSIONINFOW.
    let _ = unsafe { RtlGetVersion(&mut v) };
    v.dwBuildNumber
}

/// The best see-through effect for this Windows version: Acrylic where it drags smoothly
/// (Windows 11 22H2+), Mica on early Windows 11, Blur on Windows 10 (Acrylic lags there).
pub fn glass_effect(build: u32) -> Effect {
    match build {
        22621.. => Effect::Acrylic,
        22000..=22620 => Effect::Mica,
        _ => Effect::Blur,
    }
}

/// Applies the glass setting to the window. The page paints its own tint over the effect,
/// so the "glass tint" slider works the same with every effect.
pub fn apply_window(w: &WebviewWindow, app: &AppSettings) {
    let r = if app.glass {
        w.set_effects(
            EffectsBuilder::new()
                .effect(glass_effect(windows_build()))
                .color(Color(8, 11, 16, 0))
                .build(),
        )
    } else {
        w.set_effects(None)
    };
    if let Err(e) = r {
        eprintln!("shuusshy: window glass effect unavailable: {e}");
    }
}

// --------------------------------------------------------------------------- colour maths

pub fn rgb_to_hsl([r, g, b]: [u8; 3]) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, s.clamp(0.0, 1.0), l)
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    [to(r), to(g), to(b)]
}

/// Recolours an RGBA image rendered in [`RENDER_ACCENT`] to `accent`: rotates hue and scales
/// saturation of coloured pixels, keeping lightness and alpha (greys stay grey).
pub fn retint(rgba: &[u8], accent: [u8; 3]) -> Vec<u8> {
    let (h0, s0, _) = rgb_to_hsl(RENDER_ACCENT);
    let (h1, s1, _) = rgb_to_hsl(accent);
    let (dh, ks) = (h1 - h0, if s0 > 0.0 { s1 / s0 } else { 1.0 });
    let mut out = rgba.to_vec();
    for p in out.as_chunks_mut::<4>().0 {
        let (h, s, l) = rgb_to_hsl([p[0], p[1], p[2]]);
        if s < 0.08 {
            continue;
        }
        let rgb = hsl_to_rgb(h + dh, (s * ks).clamp(0.0, 1.0), l);
        p[..3].copy_from_slice(&rgb);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_round_trips() {
        for rgb in [
            [0x39, 0xd5, 0xff],
            [255, 0, 0],
            [12, 200, 90],
            [128, 128, 128],
            [0, 0, 0],
        ] {
            let (h, s, l) = rgb_to_hsl(rgb);
            let back = hsl_to_rgb(h, s, l);
            for i in 0..3 {
                assert!(
                    (back[i] as i32 - rgb[i] as i32).abs() <= 1,
                    "{rgb:?} -> {back:?}"
                );
            }
        }
    }

    #[test]
    fn retint_maps_render_cyan_to_the_accent_and_keeps_greys() {
        let px = [0x39, 0xd5, 0xff, 200, 90, 90, 90, 255];
        let out = retint(&px, [0x9b, 0x8c, 0xff]);
        let (h, _, _) = rgb_to_hsl([out[0], out[1], out[2]]);
        let (want, _, _) = rgb_to_hsl([0x9b, 0x8c, 0xff]);
        assert!((h - want).abs() < 2.0, "hue {h} vs {want}");
        assert_eq!(out[3], 200);
        assert_eq!(&out[4..], &[90, 90, 90, 255]);
        // Cyan accent is the identity.
        let same = retint(&px, RENDER_ACCENT);
        assert!(
            same.iter()
                .zip(&px)
                .all(|(a, b)| (*a as i32 - *b as i32).abs() <= 1)
        );
    }

    #[test]
    fn dark_accents_are_lifted_for_contrast() {
        let (_, _, l) = rgb_to_hsl(readable([0, 40, 120]));
        assert!(l >= 0.61);
        let ice = readable([0xc7, 0xd7, 0xea]);
        assert!(
            ice.iter()
                .zip([0xc7, 0xd7, 0xea])
                .all(|(a, b)| (*a as i32 - b).abs() <= 1)
        );
    }

    #[test]
    fn effect_per_windows_version() {
        assert_eq!(glass_effect(19045), Effect::Blur);
        assert_eq!(glass_effect(22000), Effect::Mica);
        assert_eq!(glass_effect(26100), Effect::Acrylic);
    }
}
