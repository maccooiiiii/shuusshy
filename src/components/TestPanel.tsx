import { useEffect, useState } from "react";
import {
  type Meters,
  type MonitorDevice,
  type MonitorSource,
  TEST_INSTRUCTIONS,
  defaultMonitorDevice,
  getMeters,
  listMonitorDevices,
  setMonitorSource,
  startMonitor,
  stopMonitor,
} from "../lib/monitor";
import type { RoutingStatus } from "../lib/routing";
import { LevelMeter } from "./LevelMeter";

interface Props {
  routing: RoutingStatus | null;
  onClose: () => void;
}

/**
 * Live test (spec Part 4): real before/after meters and headphone monitoring of either the
 * original or the processed signal. Monitoring needs an explicit click, never plays both
 * signals, and stops on Stop, Esc, or when the panel closes. Nothing is recorded.
 */
export function TestPanel({ routing, onClose }: Props) {
  const [meters, setMeters] = useState<Meters | null>(null);
  const [devices, setDevices] = useState<MonitorDevice[]>([]);
  const [deviceId, setDeviceId] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const running = routing?.state === "running";
  const monitor = routing?.monitor ?? null;
  const device = devices.find((d) => d.id === deviceId) ?? null;

  // Real meter values from the audio thread, about 20 times a second while the panel is open.
  useEffect(() => {
    let alive = true;
    const tick = () =>
      getMeters()
        .then((m) => alive && setMeters(m))
        .catch(() => {});
    tick();
    const t = window.setInterval(tick, 50);
    return () => {
      alive = false;
      window.clearInterval(t);
    };
  }, []);

  useEffect(() => {
    listMonitorDevices()
      .then((list) => {
        setDevices(list);
        setDeviceId((cur) => cur || defaultMonitorDevice(list)?.id || "");
      })
      .catch((e) => setError(String(e)));
  }, []);

  // Safety: closing the panel (or leaving the app view) always stops monitoring; Esc stops too.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") stopMonitor().catch(() => {});
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      stopMonitor().catch(() => {});
    };
  }, []);

  const listen = async (source: MonitorSource) => {
    setError(null);
    try {
      if (monitor) await setMonitorSource(source);
      else if (device) await startMonitor(device.id, source);
    } catch (e) {
      setError(String(e));
    }
  };

  const sourceButton = (source: MonitorSource, label: string) => {
    const active = monitor?.source === source;
    return (
      <button
        type="button"
        disabled={!running || !device}
        onClick={() => listen(source)}
        aria-pressed={active}
        className={`press flex-1 rounded-lg border px-2 py-1.5 text-[12px] font-medium
          ${active ? "border-accent bg-accent/20 text-ink" : "border-line text-ink/90 hover:bg-panel-hi"}
          disabled:cursor-not-allowed disabled:opacity-40`}
      >
        {label}
      </button>
    );
  };

  return (
    <div className="glass glass-live reveal flex flex-col gap-2.5 px-3 py-2.5">
      <div className="flex items-center justify-between">
        <span className="label-tech flex items-center gap-2 text-accent">
          <span aria-hidden="true" className="h-px w-3 bg-accent/70" />
          Live test
        </span>
        <button
          type="button"
          onClick={onClose}
          className="rounded px-1.5 py-0.5 text-[11px] text-muted hover:bg-panel-hi hover:text-ink"
        >
          Close
        </button>
      </div>
      <p className="text-[11px] leading-snug text-muted">{TEST_INSTRUCTIONS}</p>

      {!running && (
        <p className="text-[11px] text-warn">
          Audio is not running ({routing ? routing.state : "starting"}), so there is nothing to measure yet.
        </p>
      )}

      <LevelMeter
        label="Microphone input (before suppression)"
        rmsDb={meters?.inputRmsDb ?? null}
        peakDb={meters?.inputPeakDb ?? null}
      />
      <LevelMeter
        label="Cleaned output (what apps hear)"
        rmsDb={meters?.outputRmsDb ?? null}
        peakDb={meters?.outputPeakDb ?? null}
      />

      <div className="flex flex-col gap-1">
        <label className="text-[11px] text-muted" htmlFor="monitor-device">
          Listen on
        </label>
        <select
          id="monitor-device"
          value={deviceId}
          disabled={!!monitor}
          onChange={(e) => setDeviceId(e.target.value)}
          className="rounded-lg border border-line bg-base px-2 py-1 text-[12px] text-ink disabled:opacity-60"
        >
          {devices.map((d) => (
            <option key={d.id} value={d.id}>
              {d.name}
              {d.isDefault ? " (Windows default)" : ""}
            </option>
          ))}
        </select>
        <span className={`text-[10.5px] leading-snug ${device?.looksLikeSpeakers ? "text-warn" : "text-muted"}`}>
          {device?.looksLikeSpeakers
            ? "This looks like loudspeakers: your microphone can pick them up and howl. Use headphones, or keep the volume low."
            : "Use headphones so the microphone does not pick up what you hear."}
        </span>
      </div>

      <div className="flex gap-1.5">
        {sourceButton("original", "Hear original")}
        {sourceButton("processed", "Hear processed")}
      </div>
      {monitor && (
        <button
          type="button"
          onClick={() => stopMonitor().catch((e) => setError(String(e)))}
          className="press reveal rounded-lg border border-danger/60 bg-danger/15 px-3 py-1.5 text-[12px] font-semibold text-danger hover:bg-danger/25"
        >
          Stop listening (Esc)
        </button>
      )}

      <span className="text-[10.5px] leading-snug text-muted">
        {monitor
          ? `Playing the ${monitor.source} audio on ${monitor.device}${monitor.dropouts ? ` · ${monitor.dropouts} dropouts` : ""}.`
          : "Not listening."}{" "}
        Nothing is recorded or saved.
      </span>
      {(error || routing?.monitorError) && (
        <span className="text-[10.5px] text-danger">{error ?? routing?.monitorError}</span>
      )}
    </div>
  );
}
