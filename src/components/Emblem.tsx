interface Props {
  /** True only while the engine is really processing (from the backend status). */
  live: boolean;
  /** Display size in CSS pixels (frames are rendered at 112 px). */
  size?: number;
}

/**
 * Shuusshy's 3D emblem, rendered in Blender (tools/blender/render_emblem.py). While noise
 * cancellation is working, the voice bar pulses, the noise bars stay shushed and a ripple
 * radiates out; otherwise it shows a still, dimmed frame. Reduced motion holds the first frame.
 */
export function Emblem({ live, size = 48 }: Props) {
  return (
    <span
      aria-hidden="true"
      className={`emblem ${live ? "emblem-on" : "emblem-off"}`}
      style={{ "--size": `${size}px` } as React.CSSProperties}
    />
  );
}
