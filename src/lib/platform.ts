// Platform-specific names and wording. Shuusshy runs on Windows (VB-CABLE) and macOS (BlackHole).
// WKWebView on macOS reports a "Macintosh" user agent; WebView2 on Windows does not.

export const IS_MAC =
  (typeof navigator !== "undefined" && /Macintosh|Mac OS X/.test(navigator.userAgent)) ||
  // Development demo for the guide's macOS screenshots (src/dev/demo.ts).
  (import.meta.env.DEV && typeof location !== "undefined" && new URLSearchParams(location.search).get("demo") === "mac");

export interface PlatformText {
  /** "Windows" / "macOS" */
  os: string;
  /** Name of the virtual cable driver. */
  cable: string;
  cableUrl: string;
  /** The virtual cable's playback side (Shuusshy plays into it). */
  cablePlayback: string;
  /** The virtual cable's recording side (other apps record from it). */
  cableRecording: string;
  /** The tray / menu bar. */
  tray: string;
  /** Label of the sign-in start switch. */
  startAtLogin: string;
}

export const WINDOWS_TEXT: PlatformText = {
  os: "Windows",
  cable: "VB-CABLE",
  cableUrl: "https://vb-audio.com/Cable/",
  cablePlayback: "CABLE Input",
  cableRecording: "CABLE Output",
  tray: "the system tray",
  startAtLogin: "Start with Windows",
};

export const MAC_TEXT: PlatformText = {
  os: "macOS",
  cable: "BlackHole",
  cableUrl: "https://existential.audio/blackhole/",
  cablePlayback: "BlackHole 2ch",
  cableRecording: "BlackHole 2ch",
  tray: "the menu bar",
  startAtLogin: "Open at login",
};

export const P: PlatformText = IS_MAC ? MAC_TEXT : WINDOWS_TEXT;
