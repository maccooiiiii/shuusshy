// Processing status model (spec Part 4.5). The UI derives every indicator from one of these
// states; nothing is shown as "active" unless the backend reports it.

export type EngineStatus =
  | { kind: "unavailable"; reason: string }
  | { kind: "initializing" }
  | { kind: "processing" }
  | { kind: "bypassed" }
  | { kind: "interrupted"; reason: string }
  | { kind: "error"; reason: string };

export type Tone = "neutral" | "active" | "warning" | "error";

export interface StatusPresentation {
  label: string;
  tone: Tone;
  detail?: string;
}

export function presentStatus(status: EngineStatus): StatusPresentation {
  switch (status.kind) {
    case "unavailable":
      return { label: "Unavailable", tone: "neutral", detail: status.reason };
    case "initializing":
      return { label: "Starting…", tone: "warning" };
    case "processing":
      return { label: "Active", tone: "active" };
    case "bypassed":
      return { label: "Bypassed", tone: "neutral", detail: "Audio passes through unprocessed" };
    case "interrupted":
      return { label: "Interrupted", tone: "warning", detail: status.reason };
    case "error":
      return { label: "Error", tone: "error", detail: status.reason };
  }
}

/** Shown until the first status arrives from the backend. */
export const ENGINE_LOADING: EngineStatus = { kind: "initializing" };

/**
 * Readable text for an error from a backend command or the platform. Strips the generic
 * "Error:" prefix; the backend's own messages already say what happened and what to do.
 */
export function errorText(e: unknown): string {
  const raw = e instanceof Error ? e.message : String(e);
  const text = raw.replace(/^(Error:\s*)+/i, "").trim();
  return text || "Something went wrong. Please try again.";
}
