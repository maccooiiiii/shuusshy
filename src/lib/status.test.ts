import { describe, expect, it } from "vitest";
import { ENGINE_LOADING, errorText, presentStatus, type EngineStatus } from "./status";

describe("presentStatus", () => {
  it("only reports the active tone for the processing state", () => {
    const all: EngineStatus[] = [
      { kind: "unavailable", reason: "x" },
      { kind: "initializing" },
      { kind: "processing" },
      { kind: "bypassed" },
      { kind: "interrupted", reason: "x" },
      { kind: "error", reason: "x" },
    ];
    const active = all.filter((s) => presentStatus(s).tone === "active");
    expect(active).toEqual([{ kind: "processing" }]);
  });

  it("surfaces failure reasons", () => {
    expect(presentStatus({ kind: "error", reason: "model missing" })).toMatchObject({
      tone: "error",
      detail: "model missing",
    });
  });

  it("the loading placeholder is never presented as active", () => {
    expect(presentStatus(ENGINE_LOADING).tone).not.toBe("active");
  });
});

describe("errorText", () => {
  it("strips the generic prefix and keeps the backend message", () => {
    expect(errorText("Error: the virtual cable is busy")).toBe("the virtual cable is busy");
    expect(errorText(new Error("device lost"))).toBe("device lost");
    expect(errorText("")).toBe("Something went wrong. Please try again.");
  });
});
