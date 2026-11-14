import { describe, expect, it } from "vitest";
import {
  type RoutingStatus,
  engineDetail,
  hasGlitches,
  isolationLine,
  routingSummary,
  strengthHelp,
  telemetryLine,
} from "./routing";

const base = {
  input: null,
  output: null,
  info: null,
  stats: null,
  uptimeS: null,
  engine: { kind: "unavailable" as const, reason: "Virtual mic is off" },
  engineName: null,
  engineTelemetry: null,
  chainTelemetry: null,
  monitor: null,
  monitorError: null,
};

describe("routingSummary", () => {
  it("is only active while actually running", () => {
    const states: RoutingStatus[] = [
      { ...base, state: "off" },
      { ...base, state: "waiting", reason: "No microphone selected" },
      { ...base, state: "starting" },
      { ...base, state: "interrupted", reason: "device was disconnected" },
      { ...base, state: "error", message: "cannot open" },
      { ...base, state: "running" },
    ];
    const active = states.filter((s) => routingSummary(s).tone === "active").map((s) => s.state);
    expect(active).toEqual(["running"]);
  });

  it("shows the measured buffer and the reason for waiting", () => {
    const running: RoutingStatus = {
      ...base,
      state: "running",
      stats: {
        bufferedMs: 21.4,
        driftPpm: -36,
        underruns: 0,
        overruns: 0,
        resyncs: 0,
        xruns: 1,
        frames: 100,
        maxFrameMs: 0.2,
      },
    };
    expect(routingSummary(running).detail).toContain("21 ms");
    expect(routingSummary({ ...base, state: "waiting", reason: "VB-CABLE is not installed" }).detail).toBe(
      "VB-CABLE is not installed",
    );
    expect(hasGlitches(running.stats!)).toBe(false);
    expect(hasGlitches({ ...running.stats!, underruns: 2 })).toBe(true);
  });
});

describe("engineDetail", () => {
  it("describes processing, bypass and unavailability honestly", () => {
    const running = { ...base, state: "running" as const, engineName: "RNNoise" };
    expect(engineDetail({ ...running, engine: { kind: "processing" } }, 0.8)).toBe(
      "RNNoise · strength 80%",
    );
    expect(engineDetail({ ...running, engine: { kind: "bypassed" } }, 0.8)).toContain(
      "unprocessed",
    );
    expect(engineDetail({ ...base, state: "off" }, 0.8)).toBe("Virtual mic is off");
  });
});

describe("engine-specific text", () => {
  it("explains strength honestly per engine", () => {
    expect(strengthHelp("rnnoise")).toContain("blends");
    expect(strengthHelp("deepFilterNet")).toContain("attenuation limit");
  });
  it("shows only the figures the engine provides", () => {
    const base = { frames: 10, badFrames: 0, avgMs: 0.2, overloaded: false };
    expect(telemetryLine({ ...base, vad: 0.5, snrDb: null })).toBe(
      "0.20 ms per 10 ms frame · voice detected 50%",
    );
    expect(telemetryLine({ ...base, vad: null, snrDb: 12.4 })).toBe(
      "0.20 ms per 10 ms frame · speech-to-noise 12 dB",
    );
  });
});

describe("preset in the banner", () => {
  it("names the active preset only while processing", () => {
    const running = { ...base, state: "running" as const, engineName: "DeepFilterNet" };
    expect(engineDetail({ ...running, engine: { kind: "processing" } }, 0.8, "Balanced")).toBe(
      "Balanced · DeepFilterNet · strength 80%",
    );
    expect(engineDetail({ ...running, engine: { kind: "bypassed" } }, 0.8, "Balanced")).toContain(
      "unprocessed",
    );
  });
});

describe("isolationLine", () => {
  const c = { gateEnabled: false, gateOpen: true, limitedFrames: 0, isolationEnabled: true, voiceLevelDb: -18 };
  it("explains what is muted, from the live voice level", () => {
    expect(isolationLine(c, 6)).toBe(
      "Your voice ≈ -18 dBFS · speech below -24 dBFS is muted while you pause · gate open",
    );
  });
  it("asks for speech while the voice level is unknown, and is silent when off", () => {
    expect(isolationLine({ ...c, voiceLevelDb: -70 }, 6)).toContain("say a few words");
    expect(isolationLine({ ...c, isolationEnabled: false }, 6)).toBeNull();
  });
});
