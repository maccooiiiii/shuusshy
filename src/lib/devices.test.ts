import { describe, expect, it } from "vitest";
import {
  type AudioDevice,
  type DeviceState,
  cableConflict,
  cableSummary,
  formatLabel,
  micSummary,
  selectableMics,
} from "./devices";

const dev = (over: Partial<AudioDevice>): AudioDevice => ({
  id: "id",
  name: "Microphone (Realtek High Definition Audio)",
  description: "Microphone",
  adapter: "Realtek High Definition Audio",
  enumerator: "HDAUDIO",
  direction: "capture",
  kind: "physical",
  isDefault: false,
  format: { sampleRate: 48000, channels: 2 },
  formatError: null,
  ...over,
});

describe("device helpers", () => {
  it("only offers physical microphones", () => {
    const state: DeviceState = {
      inputs: [
        dev({ id: "mic" }),
        dev({ id: "cable", kind: "vbCable" }),
        dev({ id: "krisp", kind: "otherVirtual" }),
      ],
      mic: { state: "notSelected" },
      cable: { state: "notInstalled" },
      cableUsers: [],
      error: null,
    };
    expect(selectableMics(state).map((d) => d.id)).toEqual(["mic"]);
  });

  it("formats sample rates", () => {
    expect(formatLabel({ sampleRate: 48000, channels: 2 })).toBe("48 kHz · 2 ch");
    expect(formatLabel({ sampleRate: 44100, channels: 1 })).toBe("44.1 kHz · 1 ch");
    expect(formatLabel(null)).toBe("Format unknown");
  });

  it("summarises microphone states with honest tones", () => {
    expect(micSummary({ state: "available", device: dev({}) }).tone).toBe("active");
    expect(micSummary({ state: "notSelected" }).tone).toBe("warning");
    const gone = micSummary({ state: "disconnected", id: "x", name: "USB Mic", reason: "unplugged" });
    expect(gone).toMatchObject({ title: "USB Mic", tone: "warning" });
    expect(gone.detail).toContain("unplugged");
    expect(micSummary({ state: "error", device: dev({}), message: "in use" })).toMatchObject({
      tone: "error",
      detail: "in use",
    });
  });

  it("tells the user which device to pick in other apps", () => {
    const capture = dev({
      name: "CABLE Output (VB-Audio Virtual Cable)",
      description: "CABLE Output",
      kind: "vbCable",
    });
    const render = dev({
      name: "CABLE Input (VB-Audio Virtual Cable)",
      description: "CABLE Input",
      kind: "vbCable",
      direction: "render",
    });
    expect(cableSummary({ state: "ready", render, capture }).detail).toContain("CABLE Output");
    expect(cableSummary({ state: "incomplete", render: null, capture }).detail).toContain("CABLE Input");
    expect(cableSummary({ state: "notInstalled" }).tone).toBe("warning");
  });
});

describe("cableConflict", () => {
  it("is silent when no other app uses the cable", () => {
    expect(cableConflict([])).toBeNull();
  });
  it("names the apps and tells the user what to change", () => {
    expect(cableConflict(["Discord"])).toMatchObject({
      title: "Discord is playing into the virtual cable",
      tone: "warning",
    });
    expect(cableConflict(["Discord"])!.detail).toContain("headphones");
    expect(cableConflict(["Discord", "Zoom", "Chrome"])!.title).toBe(
      "Discord, Zoom and Chrome are playing into the virtual cable",
    );
  });
});
