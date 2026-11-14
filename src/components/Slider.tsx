import { useEffect, useRef, useState } from "react";

interface RangeProps {
  value: number;
  min: number;
  max: number;
  step: number;
  onCommit: (value: number) => void;
  format: (value: number) => string;
  label: string;
  disabled?: boolean;
}

/**
 * Range slider that moves locally while dragging and commits (debounced), so audio follows the
 * slider without writing the settings database on every pixel.
 */
export function RangeSlider({ value, min, max, step, onCommit, format, label, disabled }: RangeProps) {
  const [local, setLocal] = useState(value);
  const timer = useRef<number | undefined>(undefined);
  const dragging = useRef(false);

  useEffect(() => {
    if (!dragging.current) setLocal(value);
  }, [value]);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  const pct = ((local - min) / (max - min)) * 100;
  return (
    <div className={`flex items-center gap-2 ${disabled ? "opacity-40" : ""}`}>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={local}
        aria-label={label}
        disabled={disabled}
        onChange={(e) => {
          dragging.current = true;
          const v = Number(e.target.value);
          setLocal(v);
          window.clearTimeout(timer.current);
          timer.current = window.setTimeout(() => {
            dragging.current = false;
            onCommit(v);
          }, 150);
        }}
        className="slider h-1.5 min-w-0 flex-1 cursor-pointer appearance-none rounded-full bg-line disabled:cursor-not-allowed"
        style={{
          background: `linear-gradient(to right, var(--color-accent-dim), var(--color-accent) ${pct}%, var(--color-line) ${pct}%)`,
        }}
      />
      <span className="num w-14 text-right text-[11px] text-muted">{format(local)}</span>
    </div>
  );
}

interface SliderProps {
  /** 0..1 */
  value: number;
  onCommit: (value: number) => void;
  label: string;
  disabled?: boolean;
}

/** Percentage slider (0–100 %) over a 0..1 value. */
export function Slider({ value, onCommit, label, disabled }: SliderProps) {
  return (
    <RangeSlider
      value={Math.round(value * 100)}
      min={0}
      max={100}
      step={1}
      label={label}
      disabled={disabled}
      format={(v) => `${v}%`}
      onCommit={(v) => onCommit(v / 100)}
    />
  );
}

/** Formats decibels with an explicit sign, e.g. "+6.0 dB". */
export const formatDb = (v: number) => `${v > 0 ? "+" : ""}${v.toFixed(1)} dB`;
