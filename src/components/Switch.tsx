interface SwitchProps {
  checked: boolean;
  onChange?: (checked: boolean) => void;
  disabled?: boolean;
  label: string;
  size?: "sm" | "md";
}

/**
 * Liquid glass switch: a recessed track that fills with the accent, and a lens-like knob that
 * slides with a spring and squishes while pressed (see .liquid-switch in index.css).
 */
export function Switch({ checked, onChange, disabled, label, size = "md" }: SwitchProps) {
  const dims = size === "md" ? { w: 46, h: 26, k: 22 } : { w: 38, h: 22, k: 18 };
  const travel = dims.w - dims.k - 4;
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange?.(!checked)}
      className="liquid-switch group relative inline-flex shrink-0 items-center rounded-full p-[2px] disabled:cursor-not-allowed disabled:opacity-40"
      style={{ width: dims.w, height: dims.h }}
    >
      <span
        className="liquid-knob block rounded-full group-active:scale-x-[1.28]"
        style={{
          width: dims.k,
          height: dims.k,
          transform: `translateX(${checked ? travel : 0}px)`,
          transformOrigin: checked ? "right center" : "left center",
        }}
      />
    </button>
  );
}
