// Device types mirror src-tauri/src/audio/devices/{model.rs,mod.rs} (serde camelCase, tagged enums).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Tone } from "./status";

export type DeviceKind = "physical" | "vbCable" | "otherVirtual";
export type EndpointState = "active" | "disabled" | "notPresent" | "unplugged" | "unknown";

export interface MixFormat {
  sampleRate: number;
  channels: number;
}

export interface AudioDevice {
  id: string;
  name: string;
  description: string | null;
  adapter: string | null;
  enumerator: string | null;
  direction: "capture" | "render";
  kind: DeviceKind;
  isDefault: boolean;
  format: MixFormat | null;
  formatError: string | null;
}

export type MicStatus =
  | { state: "notSelected" }
  | { state: "available"; device: AudioDevice }
  | { state: "error"; device: AudioDevice; message: string }
  | { state: "disconnected"; id: string; name: string | null; reason: EndpointState };

export type CableStatus =
  | { state: "ready"; render: AudioDevice; capture: AudioDevice }
  | { state: "incomplete"; render: AudioDevice | null; capture: AudioDevice | null }
  | { state: "notInstalled" };

export interface DeviceState {
  inputs: AudioDevice[];
  mic: MicStatus;
  cable: CableStatus;
  /** Other apps playing into the virtual cable. */
  cableUsers: string[];
  error: string | null;
}

export const getDeviceState = () => invoke<DeviceState>("get_device_state");
export const refreshDevices = () => invoke<DeviceState>("refresh_devices");

/** Name Shuusshy can give VB-CABLE's recording endpoint, so apps list it as Shuusshy. */
export const SHUUSSHY_MIC_LABEL = "Shuusshy Microphone";

/** Renames the virtual mic in Windows (may show the administrator prompt). */
export const setVirtualMicName = (shuusshy: boolean) =>
  invoke<DeviceState>("set_virtual_mic_name", { shuusshy });
export const selectInputDevice = (id: string) =>
  invoke<DeviceState>("select_input_device", { id });
export const onDevicesChanged = (cb: (s: DeviceState) => void): Promise<UnlistenFn> =>
  listen<DeviceState>("devices-changed", (e) => cb(e.payload));

export const VB_CABLE_URL = "https://vb-audio.com/Cable/";

/** Microphones the user may select: physical capture devices only. */
export const selectableMics = (s: DeviceState) => s.inputs.filter((d) => d.kind === "physical");

export function formatLabel(f: MixFormat | null): string {
  if (!f) return "Format unknown";
  const khz = f.sampleRate / 1000;
  return `${Number.isInteger(khz) ? khz : khz.toFixed(1)} kHz · ${f.channels} ch`;
}

const reasonText: Record<EndpointState, string> = {
  unplugged: "unplugged",
  notPresent: "not connected",
  disabled: "disabled in Windows",
  active: "unavailable",
  unknown: "unavailable",
};

export interface Summary {
  title: string;
  detail: string;
  tone: Tone;
}

export function micSummary(mic: MicStatus): Summary {
  switch (mic.state) {
    case "notSelected":
      return { title: "No microphone selected", detail: "Choose a microphone", tone: "warning" };
    case "available":
      return { title: mic.device.name, detail: formatLabel(mic.device.format), tone: "active" };
    case "error":
      return { title: mic.device.name, detail: mic.message, tone: "error" };
    case "disconnected":
      return {
        title: mic.name ?? "Selected microphone",
        detail: `Microphone ${reasonText[mic.reason]}. Reconnect it or choose another.`,
        tone: "warning",
      };
  }
}

export function cableSummary(cable: CableStatus): Summary {
  switch (cable.state) {
    case "ready":
      return {
        title: "Virtual microphone detected",
        detail: `Virtual mic ready · in other apps choose “${cable.capture.description ?? cable.capture.name}”`,
        tone: "active",
      };
    case "incomplete":
      return {
        title: "VB-CABLE is incomplete",
        detail: !cable.render
          ? "“CABLE Input” playback device is missing or disabled"
          : "“CABLE Output” recording device is missing or disabled",
        tone: "warning",
      };
    case "notInstalled":
      return {
        title: "Virtual microphone not installed",
        detail: "Install VB-CABLE to use Shuusshy in other apps",
        tone: "warning",
      };
  }
}

/** Warning when other apps play into the virtual cable, or null when there are none. */
export function cableConflict(users: string[]): Summary | null {
  if (users.length === 0) return null;
  const who =
    users.length === 1 ? users[0] : `${users.slice(0, -1).join(", ")} and ${users[users.length - 1]}`;
  const verb = users.length === 1 ? "is" : "are";
  return {
    title: `${who} ${verb} playing into the virtual cable`,
    detail: `Set ${users.length === 1 ? "its" : "their"} speaker/output to your headphones — a CABLE device as output blocks Shuusshy and echoes into the virtual mic.`,
    tone: "warning",
  };
}
