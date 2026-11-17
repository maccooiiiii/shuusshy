// Development only: a demo backend for screenshots of the user guide.
//
// `npm run dev`, then open http://localhost:1420/?demo=win (or demo=mac). The page then runs
// with mocked Tauri commands and realistic sample data instead of the real audio engine.
// Options: state=nocable · view=settings · section=<Settings section title> · test=1 ·
// compact=1 · free=1. Never included in production builds (main.tsx imports it only when
// import.meta.env.DEV is true).
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

const q = new URLSearchParams(location.search);
export const DEMO = q.get("demo");

const mac = DEMO === "mac";
const free = q.get("free") === "1";
const noCable = q.get("state") === "nocable";

const dev = (id: string, name: string, desc: string, direction: "capture" | "render", kind: string, isDefault = false) => ({
  id,
  name,
  description: desc,
  adapter: null,
  enumerator: null,
  direction,
  kind,
  isDefault,
  format: { sampleRate: 48000, channels: direction === "capture" ? 1 : 2 },
  formatError: null,
});

const mic = mac
  ? dev("BuiltInMicrophoneDevice", "MacBook Pro Microphone", "MacBook Pro Microphone", "capture", "physical", true)
  : dev("{0.0.1}.{mic}", "Microphone (USB Headset)", "Microphone", "capture", "physical", true);
const capture = mac
  ? dev("BlackHole2ch_UID", "BlackHole 2ch", "BlackHole 2ch", "capture", "vbCable")
  : dev("{0.0.1}.{cable}", "CABLE Output (VB-Audio Virtual Cable)", "CABLE Output", "capture", "vbCable");
const render = mac
  ? dev("BlackHole2ch_UID", "BlackHole 2ch", "BlackHole 2ch", "render", "vbCable")
  : dev("{0.0.0}.{cable}", "CABLE Input (VB-Audio Virtual Cable)", "CABLE Input", "render", "vbCable");

const settings = {
  app: {
    alwaysOnTop: true,
    compact: q.get("compact") === "1",
    keepRunningInTray: true,
    licenseKey: free ? null : "SHUU-DEMO",
    activationPromptDismissed: true,
    glass: false,
    glassTint: 0.6,
    accent: "cyan",
    checkUpdates: true,
    autoDownloadUpdates: false,
    lastUpdateCheck: 0,
  },
  audio: {
    inputDeviceId: mic.id,
    virtualDeviceId: null,
    inputDeviceName: mic.name,
    routingEnabled: true,
    engine: free ? "rnnoise" : "deepFilterNet",
    suppressionEnabled: true,
    suppressionStrength: 0.8,
    inputGainDb: 0,
    outputGainDb: 0,
    noiseGate: { enabled: false, thresholdDb: -50 },
    voiceIsolation: { enabled: false, marginDb: 12 },
  },
};

const devices = {
  inputs: noCable ? [mic] : [mic, capture],
  mic: { state: "available", device: mic },
  cable: noCable ? { state: "notInstalled" } : { state: "ready", render, capture },
  cableUsers: [],
  error: null,
};

const fmt = (sampleRate: number, channels: number) => ({ sampleRate, channels, sampleFormat: "F32" });
const routing = () =>
  noCable
    ? {
        state: "waiting",
        reason: mac ? "BlackHole is not installed" : "VB-CABLE is not installed",
        input: null,
        output: null,
        info: null,
        stats: null,
        uptimeS: null,
        engine: { kind: "unavailable", reason: "Waiting for the virtual microphone" },
        engineName: null,
        engineTelemetry: null,
        chainTelemetry: null,
        monitor: null,
        monitorError: null,
      }
    : {
        state: "running",
        input: mic.name,
        output: render.name,
        info: { input: fmt(48000, 1), output: fmt(48000, 2), targetMarginMs: 20 },
        stats: { bufferedMs: 38.4, driftPpm: 12, underruns: 0, overruns: 0, resyncs: 0, xruns: 0, frames: 120000, maxFrameMs: 0.4 },
        uptimeS: 754,
        engine: { kind: "processing" },
        engineName: settings.audio.engine === "rnnoise" ? "RNNoise" : "DeepFilterNet",
        engineTelemetry: { frames: 120000, badFrames: 0, vad: null, snrDb: 18, avgMs: 0.31, overloaded: false },
        chainTelemetry: { gateEnabled: false, gateOpen: true, limitedFrames: 0, isolationEnabled: false, voiceLevelDb: -24 },
        monitor: null,
        monitorError: null,
      };

const presets = {
  current: "balanced",
  presets: [
    { id: "balanced", name: "Balanced", description: "Good noise removal with a natural voice.", engine: "deepFilterNet", strength: 0.8, gateEnabled: false, gateThresholdDb: -50, isolationEnabled: false, isolationMarginDb: 12 },
    { id: "strong", name: "Strong Noise Reduction", description: "For loud places.", engine: "deepFilterNet", strength: 1, gateEnabled: true, gateThresholdDb: -45, isolationEnabled: false, isolationMarginDb: 12 },
    { id: "speech", name: "Speech Priority", description: "Keeps every word.", engine: "deepFilterNet", strength: 0.6, gateEnabled: false, gateThresholdDb: -50, isolationEnabled: false, isolationMarginDb: 12 },
    { id: "lowCpu", name: "Low CPU Usage", description: "Lightest on older computers.", engine: "rnnoise", strength: 1, gateEnabled: false, gateThresholdDb: -50, isolationEnabled: false, isolationMarginDb: 12 },
  ],
};

let t = 0;
function meters() {
  t += 1;
  const wave = Math.sin(t / 6) * 4;
  return { inputRmsDb: -30 + wave, inputPeakDb: -18 + wave, outputRmsDb: -27 + wave, outputPeakDb: -16 + wave };
}

const handlers: Record<string, (args: Record<string, unknown>) => unknown> = {
  get_settings: () => settings,
  update_settings: (a) => Object.assign(settings, a.settings as object),
  reset_settings: () => settings,
  get_app_info: () => ({ version: "1.0.1", dataDir: "(demo)", settingsPersistent: true, portable: false, platform: mac ? "macos" : "windows" }),
  get_device_state: () => devices,
  refresh_devices: () => devices,
  select_input_device: () => devices,
  get_routing_status: routing,
  get_presets: () => presets,
  apply_preset: () => settings,
  get_license: () => (free ? { state: "free" } : { state: "pro", serial: 1, issued: "2026-11-14", expires: null }),
  get_release_info: () => ({
    version: "1.0.1",
    channel: "stable",
    releaseDate: "2026-11-16",
    expires: "2028-11-16",
    daysLeft: 729,
    validity: "valid",
    repo: "maccooiiiii/shuusshy",
    releasesUrl: "https://github.com/maccooiiiii/shuusshy/releases/latest",
    updatesEnabled: !mac,
  }),
  get_accent_color: () => "#39d5ff",
  get_meters: meters,
  list_monitor_devices: () => [
    { id: "hp", name: mac ? "External Headphones" : "Headphones (USB Headset)", isDefault: true, looksLikeSpeakers: false },
  ],
  get_start_with_windows: () => true,
};

export function installDemo() {
  // isTauri() reads this global; the mocks below provide the rest of the desktop bridge.
  (globalThis as unknown as { isTauri: boolean }).isTauri = true;
  mockWindows("main");
  mockIPC(
    (cmd, payload) => {
      const h = handlers[cmd];
      return h ? h((payload ?? {}) as Record<string, unknown>) : null;
    },
    { shouldMockEvents: true },
  );
  if (mac) document.documentElement.classList.add("mac");
  // The banner's default width (commands.rs BANNER_WIDTH), whatever the browser window is.
  const style = document.createElement("style");
  style.textContent = "html, body { width: 360px !important; }";
  document.head.append(style);
}

/** After the first render: open the view the screenshot needs. */
export function stageDemo() {
  const click = (sel: string) => (document.querySelector(sel) as HTMLElement | null)?.click();
  const view = q.get("view");
  const section = q.get("section");
  window.setTimeout(() => {
    if (q.get("test") === "1") {
      const b = [...document.querySelectorAll("button")].find((x) => /^\s*Test\s*$/.test(x.textContent ?? ""));
      b?.click();
    }
    if (view === "settings") click('button[aria-label="Settings"]');
    if (view === "guide") click('button[aria-label="User guide"]');
    window.setTimeout(() => {
      if (section) {
        const h = [...document.querySelectorAll("h2")].find((x) => x.textContent?.trim() === section);
        const sec = h?.closest("section");
        const shell = document.querySelector(".app-shell") as HTMLElement | null;
        const header = shell?.querySelector("header");
        if (sec && shell && header) {
          // Frame just this section under the header (padding 12 px above and below).
          shell.style.maxHeight = `${header.offsetHeight + sec.offsetHeight + 24}px`;
          sec.scrollIntoView({ block: "start" });
          const scroller = sec.parentElement;
          if (scroller) scroller.scrollTop -= 12;
        }
      }
    }, 300);
  }, 400);
}
