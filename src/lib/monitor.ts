// Live test: meters and headphone monitoring. Mirrors src-tauri/src/commands.rs and
// src-tauri/src/audio/chain.rs (Meters).
import { invoke } from "@tauri-apps/api/core";

export interface Meters {
  inputRmsDb: number;
  inputPeakDb: number;
  outputRmsDb: number;
  outputPeakDb: number;
}

export interface MonitorDevice {
  id: string;
  name: string;
  isDefault: boolean;
  looksLikeSpeakers: boolean;
}

export type MonitorSource = "original" | "processed";

export const getMeters = () => invoke<Meters | null>("get_meters");
export const listMonitorDevices = () => invoke<MonitorDevice[]>("list_monitor_devices");
export const startMonitor = (deviceId: string, source: MonitorSource) =>
  invoke<void>("start_monitor", { deviceId, source });
export const setMonitorSource = (source: MonitorSource) =>
  invoke<void>("set_monitor_source", { source });
export const stopMonitor = () => invoke<void>("stop_monitor");

/** Meter scale: -60 dBFS (empty) to 0 dBFS (full). */
export const METER_MIN_DB = -60;

/** Position (0–100 %) of a dBFS value on the meter scale. */
export function meterPercent(db: number): number {
  if (!Number.isFinite(db)) return 0;
  const p = ((db - METER_MIN_DB) / -METER_MIN_DB) * 100;
  return Math.min(100, Math.max(0, p));
}

/** Text for a meter value; anything below the scale reads as "—". */
export const meterLabel = (db: number) => (db <= METER_MIN_DB ? "—" : `${db.toFixed(0)} dB`);

/** The device to preselect: the Windows default output, else the first one. */
export function defaultMonitorDevice(devices: MonitorDevice[]): MonitorDevice | null {
  return devices.find((d) => d.isDefault) ?? devices[0] ?? null;
}

export const TEST_INSTRUCTIONS =
  "Speak, clap, tap your desk, or make a background sound to test your microphone. Compare the original and processed audio to hear the difference.";
