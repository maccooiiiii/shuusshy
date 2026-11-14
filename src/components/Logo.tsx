// Shuusshy mark: a tall centre "voice" bar flanked by bars that shrink away ("noise" fading).
export function Logo({ size = 22 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      aria-hidden="true"
      className="pointer-events-none shrink-0"
    >
      <defs>
        <linearGradient id="logo-g" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--color-accent)" />
          <stop offset="1" stopColor="var(--color-accent-2)" />
        </linearGradient>
      </defs>
      <rect width="24" height="24" rx="7" fill="url(#logo-g)" />
      <g fill="var(--color-base)">
        <rect x="4.5" y="10.5" width="2" height="3" rx="1" opacity="0.45" />
        <rect x="8" y="8" width="2" height="8" rx="1" opacity="0.75" />
        <rect x="11.5" y="5" width="2" height="14" rx="1" />
        <rect x="15" y="8" width="2" height="8" rx="1" opacity="0.75" />
        <rect x="18.5" y="10.5" width="2" height="3" rx="1" opacity="0.45" />
      </g>
    </svg>
  );
}
