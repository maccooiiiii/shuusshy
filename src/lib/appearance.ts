// Appearance helpers: accent recolouring of the Blender emblem and the glass pointer light.
import type { PointerEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import type { Accent } from "./ipc";
import { P } from "./platform";

/** The accent the emblem frames were rendered in. */
export const RENDER_ACCENT = "#39d5ff";

/** Accent choices shown in Settings > Appearance (hex values mirror appearance.rs). */
export const ACCENTS: { id: Accent; name: string; hex: string | null }[] = [
  { id: "cyan", name: "Cyan", hex: "#39d5ff" },
  { id: "violet", name: "Violet", hex: "#9b8cff" },
  { id: "mint", name: "Mint", hex: "#4de3b0" },
  { id: "amber", name: "Amber", hex: "#ffb547" },
  { id: "rose", name: "Rose", hex: "#ff6b9d" },
  { id: "ice", name: "Ice", hex: "#c7d7ea" },
  { id: "system", name: `${P.os} accent`, hex: null },
];

export function hexToHsl(hex: string): [number, number, number] {
  const n = parseInt(hex.replace("#", ""), 16);
  const r = ((n >> 16) & 255) / 255;
  const g = ((n >> 8) & 255) / 255;
  const b = (n & 255) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  const d = max - min;
  if (d < 1e-6) return [0, 0, l];
  const s = d / (1 - Math.abs(2 * l - 1));
  let h: number;
  if (max === r) h = 60 * (((g - b) / d + 6) % 6);
  else if (max === g) h = 60 * ((b - r) / d + 2);
  else h = 60 * ((r - g) / d + 4);
  return [h, Math.min(1, s), l];
}

/** CSS hue-rotate / saturate values that turn the cyan emblem into `hex`. */
export function accentFilter(hex: string): { shiftDeg: number; saturate: number } {
  const [h0, s0] = hexToHsl(RENDER_ACCENT);
  const [h1, s1] = hexToHsl(hex);
  let shift = h1 - h0;
  if (shift > 180) shift -= 360;
  if (shift < -180) shift += 360;
  return { shiftDeg: Math.round(shift), saturate: s0 > 0 ? +(s1 / s0).toFixed(2) : 1 };
}

/**
 * Whether the banner is on screen. WebView2 keeps document.hidden false when the window is
 * hidden to the tray or minimised, so the backend reports it ("banner-visible").
 */
export let bannerVisible = true;
export function watchBannerVisibility() {
  return listen<boolean>("banner-visible", (e) => {
    bannerVisible = e.payload;
  });
}

/** Moves the specular highlight of the glass panel under the pointer. */
export function glassPointer(e: PointerEvent<HTMLElement>) {
  const el = (e.target as HTMLElement).closest<HTMLElement>(".glass");
  if (!el) return;
  const r = el.getBoundingClientRect();
  el.style.setProperty("--mx", `${(((e.clientX - r.left) / r.width) * 100).toFixed(1)}%`);
  el.style.setProperty("--my", `${(((e.clientY - r.top) / r.height) * 100).toFixed(1)}%`);
}
