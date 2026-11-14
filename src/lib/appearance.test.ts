import { describe, expect, it } from "vitest";
import { accentFilter, hexToHsl } from "./appearance";

describe("accentFilter", () => {
  it("is the identity for the rendered cyan", () => {
    expect(accentFilter("#39d5ff")).toEqual({ shiftDeg: 0, saturate: 1 });
  });
  it("rotates to the accent hue the short way round", () => {
    const f = accentFilter("#ff6b9d");
    const target = hexToHsl("#ff6b9d")[0];
    const shifted = (hexToHsl("#39d5ff")[0] + f.shiftDeg + 360) % 360;
    expect(Math.abs(shifted - target)).toBeLessThan(1);
    expect(Math.abs(f.shiftDeg)).toBeLessThanOrEqual(180);
  });
  it("desaturates for pale accents", () => {
    expect(accentFilter("#c7d7ea").saturate).toBeLessThan(1);
  });
});
