// Mirrors src-tauri/src/audio/controller.rs RoutingStatus (state is flattened into the object).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Summary } from "./devices";
import { type EngineStatus, presentStatus } from "./status";

export interface StreamFormat {
  sampleRate: number;
  channels: number;
  sampleFormat: string;
}

export interface RoutingStats {
  bufferedMs: number;
  driftPpm: number;
  underruns: number;
  overruns: number;
  resyncs: number;
  xruns: number;
  frames: number;
  maxFrameMs: number;
}

type State =
  | { state: "off" }
  | { state: "waiting"; reason: string }
  | { state: "starting" }
  | { state: "running" }
  | { state: "interrupted"; reason: string }
  | { state: "error"; message: string };

export type RoutingStatus = State & {
  input: string | null;
  output: string | null;
  info: { input: StreamFormat; output: StreamFormat; targetMarginMs: number } | null;
  stats: RoutingStats | null;
  uptimeS: number | null;
  engine: EngineStatus;
  engineName: string | null;
  engineTelemetry: EngineTelemetry | null;
  chainTelemetry: ChainTelemetry | null;
  monitor: { device: string; source: "original" | "processed"; dropouts: number } | null;
  monitorError: string | null;
};

export interface ChainTelemetry {
  gateEnabled: boolean;
  gateOpen: boolean;
  /** Frames in which the safety limiter reduced the level by more than 1 dB. */
  limitedFrames: number;
  isolationEnabled: boolean;
  /** Tracked level of the user's own voice (dBFS). */
  voiceLevelDb: number;
}

/** Live voice-isolation readout, or null when isolation is off or the voice is not known yet. */
export function isolationLine(c: ChainTelemetry, marginDb: number): string | null {
  if (!c.isolationEnabled) return null;
  if (c.voiceLevelDb <= -59) return "Learning your voice level — say a few words.";
  const cut = c.voiceLevelDb - marginDb;
  return `Your voice ≈ ${c.voiceLevelDb.toFixed(0)} dBFS · speech below ${cut.toFixed(0)} dBFS is muted while you pause · gate ${c.gateOpen ? "open" : "closed"}`;
}

export interface EngineTelemetry {
  frames: number;
  badFrames: number;
  /** RNNoise: voice-activity probability 0..1. */
  vad: number | null;
  /** DeepFilterNet: local speech-to-noise estimate in dB. */
  snrDb: number | null;
  /** Average engine time per 10 ms frame. */
  avgMs: number;
  /** The engine could not keep up and passes audio through. */
  overloaded: boolean;
}

/** Engine-specific explanation of the strength control. */
export function strengthHelp(engine: "rnnoise" | "deepFilterNet"): string {
  return engine === "rnnoise"
    ? "RNNoise has no strength setting of its own, so Shuusshy blends cleaned and original audio: 100% is full RNNoise, lower values keep more of the room sound."
    : "Strength sets DeepFilterNet's own attenuation limit: 0% leaves audio untouched, 50% removes up to about 22 dB of noise, 100% removes as much as it can.";
}

/** Live engine figures for the settings panel. */
export function telemetryLine(t: EngineTelemetry): string {
  const parts = [`${t.avgMs.toFixed(2)} ms per 10 ms frame`];
  if (t.vad !== null) parts.push(`voice detected ${Math.round(t.vad * 100)}%`);
  if (t.snrDb !== null) parts.push(`speech-to-noise ${t.snrDb.toFixed(0)} dB`);
  if (t.badFrames > 0) parts.push(`${t.badFrames} unusable frames replaced`);
  return parts.join(" · ");
}

/** One-line description under the noise-cancellation switch. */
export function engineDetail(r: RoutingStatus, strength: number, preset?: string | null): string {
  switch (r.engine.kind) {
    case "processing": {
      const what = `${r.engineName ?? "Engine"} · strength ${Math.round(strength * 100)}%`;
      return preset ? `${preset} · ${what}` : what;
    }
    case "bypassed":
      return "Off — your microphone passes through unprocessed";
    default: {
      const p = presentStatus(r.engine);
      return p.detail ?? p.label;
    }
  }
}

export const getRoutingStatus = () => invoke<RoutingStatus>("get_routing_status");
export const onRoutingChanged = (cb: (s: RoutingStatus) => void): Promise<UnlistenFn> =>
  listen<RoutingStatus>("routing-changed", (e) => cb(e.payload));

export function routingSummary(r: RoutingStatus): Summary {
  switch (r.state) {
    case "running": {
      const ms = r.stats ? ` · ${Math.round(r.stats.bufferedMs)} ms buffer` : "";
      return { title: "Virtual mic live", detail: `Apps hear you through the virtual mic${ms}`, tone: "active" };
    }
    case "starting":
      return { title: "Starting virtual mic…", detail: "Opening audio devices", tone: "warning" };
    case "off":
      return { title: "Virtual mic off", detail: "Other apps receive no audio from Shuusshy", tone: "neutral" };
    case "waiting":
      return { title: "Virtual mic waiting", detail: r.reason, tone: "warning" };
    case "interrupted":
      return { title: "Virtual mic interrupted", detail: `${r.reason} — retrying`, tone: "warning" };
    case "error":
      return { title: "Virtual mic error", detail: `${r.message} — retrying`, tone: "error" };
  }
}

/** True when audio problems have been counted since routing started. */
export const hasGlitches = (s: RoutingStats) => s.underruns + s.overruns + s.resyncs > 0;

