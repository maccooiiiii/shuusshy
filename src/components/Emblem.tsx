import { useEffect, useRef } from "react";
import { bannerVisible } from "../lib/appearance";

interface Props {
  /** True only while the engine is really processing (from the backend status). */
  live: boolean;
  /** Display size in CSS pixels (frames are rendered at 112 px). */
  size?: number;
}

const FRAMES = 36;
/** The Blender loop is 36 frames over 2 s. */
const FPS = 18;

/**
 * Shuusshy's 3D emblem, rendered in Blender (tools/blender/render_emblem.py). While noise
 * cancellation is working, the voice bar pulses, the noise bars stay shushed and a ripple
 * radiates out; otherwise it shows a still, dimmed frame.
 *
 * Performance (measured in Milestone 11): every frame the transparent glass window produces
 * is expensive, so the strip is stepped by a timer exactly 18 times a second (a CSS animation
 * makes WebView2 produce up to 60 frames a second). It pauses while the banner is hidden or
 * minimised (backend "banner-visible" event) and
 * holds the first frame for reduced motion.
 */
export function Emblem({ live, size = 48 }: Props) {
  const strip = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const el = strip.current;
    if (!live || !el) return;
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
    let frame = 0;
    const tick = () => {
      if (document.hidden || !bannerVisible || reduced.matches) return;
      frame = (frame + 1) % FRAMES;
      el.style.transform = `translateX(${-frame * size}px)`;
    };
    const timer = window.setInterval(tick, 1000 / FPS);
    return () => {
      window.clearInterval(timer);
      el.style.transform = "";
    };
  }, [live, size]);

  return (
    <span
      aria-hidden="true"
      className={`emblem ${live ? "emblem-on" : "emblem-off"}`}
      style={{ "--size": `${size}px` } as React.CSSProperties}
    >
      <span ref={strip} className="emblem-strip" />
    </span>
  );
}
