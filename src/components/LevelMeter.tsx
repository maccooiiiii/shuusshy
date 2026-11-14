import { meterLabel, meterPercent } from "../lib/monitor";

interface Props {
  label: string;
  rmsDb: number | null;
  peakDb: number | null;
}

/**
 * Horizontal level meter: RMS as the filled bar, peak hold as a tick. Values come from the
 * audio thread's real measurements; with no data the meter is empty (never simulated).
 */
export function LevelMeter({ label, rmsDb, peakDb }: Props) {
  const rms = rmsDb === null ? 0 : meterPercent(rmsDb);
  const peak = peakDb === null ? 0 : meterPercent(peakDb);
  const hot = peakDb !== null && peakDb > -3;
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-baseline justify-between text-[11px]">
        <span className="text-ink/90">{label}</span>
        <span className="num text-muted">
          {rmsDb === null ? "no audio" : meterLabel(rmsDb)}
        </span>
      </div>
      <div
        className="well relative h-2 overflow-hidden rounded-full"
        role="meter"
        aria-label={label}
        aria-valuemin={-60}
        aria-valuemax={0}
        aria-valuenow={rmsDb ?? -60}
      >
        <div
          className={`absolute inset-y-0 left-0 rounded-full transition-[width] duration-75 ${hot ? "bg-warn" : "bg-gradient-to-r from-accent-dim to-accent shadow-[0_0_8px] shadow-accent/50"}`}
          style={{ width: `${rms}%` }}
        />
        {peak > 0 && (
          <div
            className={`absolute inset-y-0 w-0.5 ${hot ? "bg-danger" : "bg-ink/80"}`}
            style={{ left: `calc(${peak}% - 1px)` }}
          />
        )}
      </div>
    </div>
  );
}
