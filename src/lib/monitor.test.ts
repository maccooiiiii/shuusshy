import { describe, expect, it } from "vitest";
import { defaultMonitorDevice, meterLabel, meterPercent } from "./monitor";

describe("meter scale", () => {
  it("maps -60..0 dBFS to 0..100 % and clamps", () => {
    expect(meterPercent(-60)).toBe(0);
    expect(meterPercent(-30)).toBe(50);
    expect(meterPercent(0)).toBe(100);
    expect(meterPercent(-100)).toBe(0);
    expect(meterPercent(6)).toBe(100);
    expect(meterPercent(Number.NaN)).toBe(0);
  });
  it("labels silence as a dash, never as a fake number", () => {
    expect(meterLabel(-100)).toBe("—");
    expect(meterLabel(-23.4)).toBe("-23 dB");
  });
});

describe("defaultMonitorDevice", () => {
  const d = (id: string, isDefault: boolean) => ({ id, name: id, isDefault, looksLikeSpeakers: false });
  it("prefers the Windows default output", () => {
    expect(defaultMonitorDevice([d("a", false), d("b", true)])?.id).toBe("b");
    expect(defaultMonitorDevice([d("a", false)])?.id).toBe("a");
    expect(defaultMonitorDevice([])).toBeNull();
  });
});
