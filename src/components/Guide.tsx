// The in-app user guide: an interactive manual for Windows and macOS.
//
// - A platform switch (Windows / macOS), opened on the platform Shuusshy runs on.
// - "Getting started" is a live checklist on this computer: steps tick themselves when Shuusshy
//   detects the virtual cable, the microphone and the running noise cancellation, and buttons
//   jump to the place that fixes a missing step.
// - Screenshots of the real interface (src/assets/guide), tap to enlarge.
// - Troubleshooting questions open and close like an accordion.
import { useEffect, useState } from "react";
import type { DeviceState } from "../lib/devices";
import { IS_MAC, MAC_TEXT, WINDOWS_TEXT, type PlatformText } from "../lib/platform";
import type { RoutingStatus } from "../lib/routing";
import { ChevronIcon } from "./icons";

type Os = "windows" | "macos";

const SHOTS = import.meta.glob("../assets/guide/*.webp", {
  eager: true,
  query: "?url",
  import: "default",
}) as Record<string, string>;

const shot = (os: Os, name: string): string | undefined =>
  SHOTS[`../assets/guide/${os === "macos" ? "mac" : "win"}-${name}.webp`];

interface Props {
  devices: DeviceState | null;
  routing: RoutingStatus | null;
  suppressionOn: boolean;
  onOpenSettings: () => void;
  onOpenTest: () => void;
  onOpenCableSite: () => void;
  onTurnOn: () => void;
}

interface Ctx extends Props {
  os: Os;
  t: PlatformText;
  /** Viewing the guide for the platform Shuusshy runs on (live checks apply). */
  here: boolean;
  zoom: (src: string, alt: string) => void;
  done: Record<string, boolean>;
  toggleDone: (id: string) => void;
}

interface Chapter {
  id: string;
  title: string;
  summary: string;
  body: (c: Ctx) => React.ReactNode;
}

// --------------------------------------------------------------------------- building blocks

function Shot({ c, name, alt }: { c: Ctx; name: string; alt: string }) {
  const src = shot(c.os, name);
  if (!src) return null;
  return (
    <figure className="flex flex-col gap-1">
      <button
        type="button"
        onClick={() => c.zoom(src, alt)}
        className="press overflow-hidden rounded-xl border border-white/10 bg-base/40 hover:border-accent/50"
        title="Enlarge"
      >
        <img src={src} alt={alt} loading="lazy" className="block w-full" draggable={false} />
      </button>
      <figcaption className="text-center text-[10px] text-muted/80">{alt} · tap to enlarge</figcaption>
    </figure>
  );
}

const P_ = ({ children }: { children: React.ReactNode }) => (
  <p className="text-[11.5px] leading-relaxed text-ink/85">{children}</p>
);

function Steps({ items }: { items: React.ReactNode[] }) {
  return (
    <ol className="flex flex-col gap-1.5">
      {items.map((it, i) => (
        <li key={i} className="flex gap-2 text-[11.5px] leading-relaxed text-ink/85">
          <span className="num grid h-4.5 w-4.5 shrink-0 place-items-center rounded-full bg-accent/15 text-[9.5px] font-semibold text-accent">
            {i + 1}
          </span>
          <span className="min-w-0">{it}</span>
        </li>
      ))}
    </ol>
  );
}

function Tip({ children }: { children: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-accent/30 bg-accent/10 px-2.5 py-1.5 text-[11px] leading-snug text-ink/90">
      {children}
    </div>
  );
}

function Terms({ items }: { items: [string, React.ReactNode][] }) {
  return (
    <dl className="flex flex-col gap-1.5">
      {items.map(([k, v]) => (
        <div key={k} className="flex flex-col">
          <dt className="label-tech !text-[9.5px] text-accent">{k}</dt>
          <dd className="text-[11.5px] leading-snug text-ink/85">{v}</dd>
        </div>
      ))}
    </dl>
  );
}

function ActionButton({ onClick, children }: { onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="press glass-btn-accent self-start rounded-full px-2.5 py-1 text-[11px] text-ink"
    >
      {children}
    </button>
  );
}

const B = ({ children }: { children: React.ReactNode }) => (
  <strong className="font-semibold text-ink">{children}</strong>
);

// --------------------------------------------------------------------------- getting started

interface Check {
  id: string;
  title: string;
  how: React.ReactNode;
  /** Live state on this computer; null = the user ticks it. */
  live: boolean | null;
  action?: React.ReactNode;
}

function checks(c: Ctx): Check[] {
  const { t } = c;
  const cableReady = c.devices?.cable.state === "ready";
  const micOk = c.devices?.mic.state === "available";
  const processing = c.routing?.engine.kind === "processing";
  return [
    {
      id: "cable",
      title: `Install ${t.cable}`,
      how:
        c.os === "macos"
          ? "Download the free “BlackHole 2ch” installer and run it once. It adds the virtual cable that carries your voice to other apps."
          : "Download VB-CABLE (free), extract the ZIP and run VBCABLE_Setup_x64.exe as administrator. Restart if asked.",
      live: cableReady,
      action: <ActionButton onClick={c.onOpenCableSite}>Open {t.cable} download page</ActionButton>,
    },
    {
      id: "mic",
      title: "Choose your microphone",
      how: "Tap the microphone line at the top of the banner and pick your headset or built-in mic.",
      live: micOk,
      action: <ActionButton onClick={c.onOpenSettings}>Open microphone settings</ActionButton>,
    },
    {
      id: "on",
      title: "Turn on noise cancellation",
      how: "Switch on “Noise cancellation” on the banner. The emblem glows and the status says Active while it works.",
      live: processing,
      action: c.suppressionOn ? undefined : <ActionButton onClick={c.onTurnOn}>Turn it on</ActionButton>,
    },
    {
      id: "app",
      title: "Pick the virtual mic in your call app",
      how: (
        <>
          In Teams, Discord, Zoom or Meet, open audio settings and choose{" "}
          <B>
            {c.os === "macos" ? "BlackHole 2ch" : "CABLE Output (or Shuusshy Microphone if renamed)"}
          </B>{" "}
          as the microphone. Keep your speakers on your headphones, not on the cable.
        </>
      ),
      live: null,
    },
    {
      id: "test",
      title: "Test it",
      how: "Press Test on the banner, speak and make a noise, then listen to Original and Processed on your headphones.",
      live: null,
      action: <ActionButton onClick={c.onOpenTest}>Open live test</ActionButton>,
    },
  ];
}

function Checklist({ c }: { c: Ctx }) {
  const list = checks(c);
  const ok = (k: Check) => (c.here && k.live !== null ? k.live : !!c.done[k.id]);
  const count = list.filter(ok).length;
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <div className="well h-1.5 flex-1 overflow-hidden rounded-full" role="progressbar" aria-valuemin={0} aria-valuemax={list.length} aria-valuenow={count}>
          <div
            className="h-full rounded-full bg-gradient-to-r from-accent-dim to-accent transition-[width] duration-500"
            style={{ width: `${(count / list.length) * 100}%` }}
          />
        </div>
        <span className="num text-[10.5px] text-muted">
          {count}/{list.length}
        </span>
      </div>
      {!c.here && (
        <span className="text-[10.5px] text-muted">
          You are reading the {c.t.os} guide on {IS_MAC ? "a Mac" : "Windows"}: tick the steps yourself.
        </span>
      )}
      <ul className="flex flex-col gap-1.5">
        {list.map((k) => {
          const done = ok(k);
          const auto = c.here && k.live !== null;
          return (
            <li key={k.id} className={`glass flex flex-col gap-1.5 px-2.5 py-2 ${done ? "glass-live" : ""}`}>
              <div className="flex items-start gap-2">
                <button
                  type="button"
                  role="checkbox"
                  aria-checked={done}
                  disabled={auto}
                  onClick={() => c.toggleDone(k.id)}
                  title={auto ? "Checked automatically by Shuusshy" : "Mark as done"}
                  className={`press mt-0.5 grid h-4.5 w-4.5 shrink-0 place-items-center rounded-full border text-[10px]
                    ${done ? "border-accent bg-accent text-base" : "border-white/30 text-transparent"}
                    ${auto ? "cursor-default" : "hover:border-accent"}`}
                >
                  ✓
                </button>
                <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className={`text-[12px] ${done ? "text-accent" : "text-ink"}`}>{k.title}</span>
                  <span className="text-[11px] leading-snug text-muted">{k.how}</span>
                  {auto && (
                    <span className={`text-[10px] ${done ? "text-accent/80" : "text-warn"}`}>
                      {done ? "Detected by Shuusshy" : "Not detected yet"}
                    </span>
                  )}
                </div>
              </div>
              {!done && k.action && <div className="pl-6.5">{k.action}</div>}
            </li>
          );
        })}
      </ul>
      {count === list.length && (
        <Tip>All set. Your call apps now hear your voice without the background noise.</Tip>
      )}
    </div>
  );
}

// --------------------------------------------------------------------------- troubleshooting

function Faq({ items }: { items: [string, React.ReactNode][] }) {
  const [open, setOpen] = useState<number | null>(0);
  return (
    <div className="flex flex-col gap-1.5">
      {items.map(([q, a], i) => (
        <div key={q} className="glass overflow-hidden">
          <button
            type="button"
            aria-expanded={open === i}
            onClick={() => setOpen(open === i ? null : i)}
            className="press flex w-full items-center gap-2 px-2.5 py-2 text-left text-[12px] text-ink hover:bg-white/[0.04]"
          >
            <span className="flex-1">{q}</span>
            <span className="text-muted">
              <ChevronIcon open={open === i} />
            </span>
          </button>
          {open === i && (
            <div className="reveal border-t border-white/[0.07] px-2.5 py-2 text-[11.5px] leading-relaxed text-ink/85">
              {a}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

// --------------------------------------------------------------------------- chapters

const CHAPTERS: Chapter[] = [
  {
    id: "start",
    title: "Getting started",
    summary: "Five steps from installing to your first clean call",
    body: (c) => (
      <>
        <P_>
          Shuusshy cleans your microphone on this {c.os === "macos" ? "Mac" : "PC"} and passes your
          voice to other apps through a virtual microphone. Nothing is recorded or uploaded.
        </P_>
        <Checklist c={c} />
      </>
    ),
  },
  {
    id: "install",
    title: "Installing",
    summary: "Installer, first launch and the virtual cable",
    body: (c) =>
      c.os === "macos" ? (
        <>
          <P_>Shuusshy runs on macOS 11 or later, on Apple Silicon and Intel Macs.</P_>
          <Steps
            items={[
              <>Open the <B>.dmg</B> and drag <B>Shuusshy</B> onto <B>Applications</B>.</>,
              <>
                First launch: the app is not signed with an Apple Developer ID, so macOS blocks it
                once. On macOS 15 or later, double-click it, click <B>Done</B>, then go to{" "}
                <B>System Settings → Privacy &amp; Security</B> and click <B>Open Anyway</B>. On
                macOS 11–14, Control-click the app → <B>Open</B> → <B>Open</B>.
              </>,
              <>When asked, <B>Allow</B> microphone access (later: Privacy &amp; Security → Microphone).</>,
              <>
                Install <B>BlackHole 2ch</B> from existential.audio (free). Shuusshy finds it by
                itself; no restart needed.
              </>,
            ]}
          />
          <Tip>
            If macOS says the app “is damaged”, run{" "}
            <code className="select-text">xattr -dr com.apple.quarantine /Applications/Shuusshy.app</code>{" "}
            in Terminal, then open it again.
          </Tip>
          <Shot c={c} name="setup" alt="Audio Output before BlackHole is installed" />
        </>
      ) : (
        <>
          <P_>Shuusshy runs on Windows 10 and 11 (64-bit) and needs no administrator rights.</P_>
          <Steps
            items={[
              <>Run <B>Shuusshy_x.y.z_x64-setup.exe</B>, accept the terms and finish.</>,
              <>
                If SmartScreen warns (the installer is not code-signed yet), choose{" "}
                <B>More info → Run anyway</B>.
              </>,
              <>
                Install <B>VB-CABLE</B> from vb-audio.com: extract the ZIP, run{" "}
                <B>VBCABLE_Setup_x64.exe</B> as administrator and restart if asked.
              </>,
              <>Shuusshy detects the cable by itself; no restart of Shuusshy needed.</>,
            ]}
          />
          <Tip>
            Prefer no installation? The <B>portable ZIP</B> runs from any folder and keeps its
            settings in a <B>data</B> folder next to shuusshy.exe.
          </Tip>
          <Shot c={c} name="setup" alt="Audio Output before VB-CABLE is installed" />
        </>
      ),
  },
  {
    id: "banner",
    title: "The banner",
    summary: "Your microphone, the on/off switch, engine and strength",
    body: (c) => (
      <>
        <P_>The small floating banner holds everything you need during a call.</P_>
        <Shot c={c} name="main" alt="The banner while noise cancellation is active" />
        <Terms
          items={[
            ["Status", "Top left: says Active, with a glow, only while noise is really being removed."],
            ["Microphone", "Tap to choose another microphone. A dot shows whether it is connected."],
            ["Virtual mic line", "“Virtual mic live” means apps are receiving your cleaned voice."],
            ["Noise cancellation", "The main switch. Off passes your voice through untouched."],
            ["Engine", "RNNoise (free, lightest) or DeepFilterNet (Pro, strongest)."],
            ["Strength", "How much noise to remove. Lower it if your voice sounds thin."],
            ["Compact / expand", "The arrows button shrinks the banner to the essentials."],
          ]}
        />
        <Shot c={c} name="compact" alt="Compact banner" />
      </>
    ),
  },
  {
    id: "test",
    title: "Live test",
    summary: "Hear and measure before and after, on your headphones",
    body: (c) => (
      <>
        <Steps
          items={[
            <>Press <B>Test</B> on the banner.</>,
            <>Speak, clap or tap your desk: the meters show your real input and output levels.</>,
            <>Pick your <B>headphones</B> and press <B>Original</B> or <B>Processed</B> to listen.</>,
            <>Press <B>Stop</B> or Esc to stop listening. Nothing is ever recorded.</>,
          ]}
        />
        <Tip>Use headphones, not speakers: speakers feed your own voice back into the microphone.</Tip>
        <ActionButton onClick={c.onOpenTest}>Open live test now</ActionButton>
        <Shot c={c} name="test" alt="Live test with level meters" />
      </>
    ),
  },
  {
    id: "apps",
    title: "Using it in your apps",
    summary: `Teams, Discord, Zoom, Meet and more`,
    body: (c) => (
      <>
        <P_>
          In every call or recording app, choose the virtual mic as the <B>microphone</B>:
        </P_>
        <Terms
          items={
            c.os === "macos"
              ? [
                  ["Choose as microphone", "BlackHole 2ch"],
                  ["Keep as speaker", "Your headphones or Mac speakers (never BlackHole)"],
                ]
              : [
                  ["Choose as microphone", "CABLE Output (VB-Audio Virtual Cable) — or Shuusshy Microphone"],
                  ["Keep as speaker", "Your headphones (never CABLE Input)"],
                ]
          }
        />
        <Steps
          items={[
            <>Teams: Settings → Devices → Microphone.</>,
            <>Discord: User Settings → Voice &amp; Video → Input Device.</>,
            <>Zoom: Settings → Audio → Microphone.</>,
            <>Google Meet: ⋮ → Settings → Audio → Microphone.</>,
          ]}
        />
        {c.os === "windows" && (
          <Tip>
            Settings → Audio Output can rename the virtual mic to “Shuusshy Microphone” so it is
            easy to find (Windows asks for permission).
          </Tip>
        )}
        <Tip>Turn off the app’s own noise suppression, or set it to Low, so the two don’t fight.</Tip>
        <Shot c={c} name="output" alt="Settings: Audio Output with the virtual mic ready" />
      </>
    ),
  },
  {
    id: "settings",
    title: "Settings tour",
    summary: "Microphone, presets, gain, appearance and startup",
    body: (c) => (
      <>
        <P_>Open Settings with the gear button. The sections:</P_>
        <Terms
          items={[
            ["Microphone", "Choose the microphone and its input gain. Keep gain near 0 dB."],
            ["Noise Suppression", "Presets (Balanced, Strong, Speech Priority, Low CPU), engine, strength, noise gate and voice isolation (Pro)."],
            ["Audio Output", `The ${c.t.cable} status, sending to the virtual mic on or off, and output gain.`],
            ["Appearance", "See-through glass, tint and accent colours."],
            ["Startup and Behavior", `${c.t.startAtLogin}, keep on top, and keep running in ${c.t.tray} when closed.`],
            ["Application and Updates", "Your Pro key, update checks, settings backup and the release’s validity date."],
          ]}
        />
        <ActionButton onClick={c.onOpenSettings}>Open Settings</ActionButton>
        <Shot c={c} name="mic" alt="Settings: Microphone" />
        <Shot c={c} name="noise" alt="Settings: Noise Suppression and presets" />
        <Shot c={c} name="appearance" alt="Settings: Appearance" />
        <Shot c={c} name="startup" alt="Settings: Startup and Behavior" />
      </>
    ),
  },
  {
    id: "tray",
    title: "System tray",
    summary: "Keep Shuusshy running out of the way",
    body: (c) => (
      <>
        <P_>
          Shuusshy keeps an animated icon in {c.t.tray} that moves while noise cancellation is
          working.
        </P_>
        <Terms
          items={[
            ["Click the icon", "Shows or hides the banner."],
            [c.os === "macos" ? "Right-click / Control-click" : "Right-click", "Menu: open, noise cancellation on/off, settings, and quit."],
            ["Close button", `With “keep running” on, closing hides the banner to ${c.t.tray}; your voice stays clean.`],
            [c.t.startAtLogin, `Starts quietly in ${c.t.tray} when you sign in.`],
          ]}
        />
        {c.os === "macos" && <Tip>Clicking Shuusshy in the Dock also brings the banner back.</Tip>}
      </>
    ),
  },
  {
    id: "pro",
    title: "Shuusshy Pro",
    summary: "DeepFilterNet and voice isolation",
    body: (c) => (
      <>
        <P_>
          RNNoise is free forever. A Pro key unlocks <B>DeepFilterNet</B> (the strongest engine),{" "}
          <B>voice isolation</B> (mutes quieter background talkers while you pause) and the presets
          built on them.
        </P_>
        <Steps
          items={[
            <>Open <B>Settings → Application and Updates</B>.</>,
            <>Paste your key (it starts with SHUU-) and press <B>Activate</B>.</>,
            <>The key is checked offline on this computer: no account, no internet.</>,
          ]}
        />
        <ActionButton onClick={c.onOpenSettings}>Enter a key in Settings</ActionButton>
        <Shot c={c} name="app" alt="Settings: Application and Updates" />
      </>
    ),
  },
  {
    id: "updates",
    title: "Updates and validity",
    summary: "New versions and the two-year validity",
    body: (c) => (
      <>
        {c.os === "macos" ? (
          <P_>
            The Mac version does not update itself yet: download the new .dmg from the Releases
            page and drag it to Applications again. Your settings are kept.
          </P_>
        ) : (
          <P_>
            Shuusshy checks GitHub for a new version at most once a day (you can turn it off).
            Updates are signature-checked and installed only when you click. Your settings are
            kept.
          </P_>
        )}
        <P_>
          Each official release is valid for two years. You get a reminder 30 days before and a
          warning 7 days before; after that, noise cancellation stops until you install a newer
          version. Settings → Application and Updates shows the date.
        </P_>
      </>
    ),
  },
  {
    id: "help",
    title: "Troubleshooting",
    summary: "Fixes for the most common problems",
    body: (c) => (
      <Faq
        items={[
          [
            "Other apps don’t hear me",
            <>
              Check that the call app’s microphone is{" "}
              <B>{c.os === "macos" ? "BlackHole 2ch" : "CABLE Output / Shuusshy Microphone"}</B>,
              that “Send microphone to virtual mic” is on (Settings → Audio Output), and that the
              banner says “Virtual mic live”.
            </>,
          ],
          [
            `Shuusshy says ${c.t.cable} is not installed`,
            c.os === "macos" ? (
              <>Install “BlackHole 2ch” (not 16ch). If it still isn’t found, open Audio MIDI Setup and check that BlackHole 2ch is listed.</>
            ) : (
              <>Install VB-CABLE as administrator and restart Windows. Then check that CABLE Input and CABLE Output are enabled in Sound settings.</>
            ),
          ],
          [
            "My voice sounds robotic or choppy",
            <>
              Lower the strength or try Speech Priority. Keep Shuusshy’s gains near 0 dB and set the
              microphone level in{" "}
              {c.os === "macos" ? "System Settings → Sound → Input" : "Windows Sound settings (and turn off “audio enhancements”)"}.
              A distorted microphone stays distorted after noise removal.
            </>,
          ],
          [
            "I hear an echo, or people hear their own voice",
            <>An app is playing into the virtual cable. Set every app’s speaker/output to your headphones, never to {c.os === "macos" ? "BlackHole" : "a CABLE device"}.</>,
          ],
          [
            "The microphone shows as disconnected",
            <>Plug it back in or choose another in the microphone list. Shuusshy reconnects by itself when it comes back.</>,
          ],
          ...(c.os === "macos"
            ? ([
                [
                  "The microphone is silent on Mac",
                  <>macOS may have blocked access: System Settings → Privacy &amp; Security → Microphone → turn on Shuusshy, then restart Shuusshy.</>,
                ],
              ] as [string, React.ReactNode][])
            : []),
          [
            "DeepFilterNet is greyed out",
            <>It is part of Shuusshy Pro. Enter your key in Settings → Application and Updates.</>,
          ],
        ]}
      />
    ),
  },
];

// --------------------------------------------------------------------------- the guide

const DONE_KEY = "shuusshy-guide-done";

export function Guide(props: Props) {
  const [os, setOs] = useState<Os>(IS_MAC ? "macos" : "windows");
  const [chapter, setChapter] = useState<string | null>(null);
  const [zoomed, setZoomed] = useState<{ src: string; alt: string } | null>(null);
  const [done, setDone] = useState<Record<string, boolean>>(() => {
    try {
      return JSON.parse(localStorage.getItem(DONE_KEY) ?? "{}");
    } catch {
      return {};
    }
  });
  useEffect(() => {
    localStorage.setItem(DONE_KEY, JSON.stringify(done));
  }, [done]);
  useEffect(() => {
    if (!zoomed) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setZoomed(null);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [zoomed]);

  const ctx: Ctx = {
    ...props,
    os,
    t: os === "macos" ? MAC_TEXT : WINDOWS_TEXT,
    here: (os === "macos") === IS_MAC,
    zoom: (src, alt) => setZoomed({ src, alt }),
    done,
    toggleDone: (id) => setDone((d) => ({ ...d, [`${os}:${id}`]: !d[`${os}:${id}`] })),
  };
  // Manual ticks are kept per platform.
  ctx.done = Object.fromEntries(
    Object.entries(done)
      .filter(([k]) => k.startsWith(`${os}:`))
      .map(([k, v]) => [k.slice(os.length + 1), v]),
  );
  const index = CHAPTERS.findIndex((ch) => ch.id === chapter);
  const current = index >= 0 ? CHAPTERS[index] : null;
  const title = (ch: Chapter) => (ch.id === "tray" ? (os === "macos" ? "Menu bar" : "System tray") : ch.title);

  return (
    <main className="enter-right scroll-thin relative flex min-h-0 flex-col gap-3 overflow-y-auto px-3 py-3 text-[13px]">
      <div role="tablist" aria-label="Platform" className="well relative flex rounded-full p-[3px]">
        <span
          aria-hidden="true"
          className="liquid-lens absolute inset-y-[3px] left-[3px] w-[calc(50%-3px)] rounded-full transition-transform duration-300"
          style={{ transform: os === "macos" ? "translateX(100%)" : "none" }}
        />
        {(["windows", "macos"] as Os[]).map((o) => (
          <button
            key={o}
            type="button"
            role="tab"
            aria-selected={os === o}
            onClick={() => setOs(o)}
            className={`press relative z-10 flex-1 rounded-full px-2 py-1 text-[11.5px] transition-colors
              ${os === o ? "text-ink" : "text-muted hover:text-ink"}`}
          >
            {o === "macos" ? "macOS" : "Windows"}
            {(o === "macos") === IS_MAC && <span className="ml-1 text-[9.5px] text-accent">· this computer</span>}
          </button>
        ))}
      </div>

      {current ? (
        <article key={`${os}-${current.id}`} className="reveal flex flex-col gap-2.5">
          <button
            type="button"
            onClick={() => setChapter(null)}
            className="press self-start text-[11px] text-muted hover:text-ink"
          >
            ← All topics
          </button>
          <div className="flex flex-col">
            <span className="label-tech !text-[9.5px] text-accent">
              {index + 1} of {CHAPTERS.length}
            </span>
            <h2 className="font-display text-[16px] font-semibold tracking-tight">{title(current)}</h2>
          </div>
          {current.body(ctx)}
          <nav className="mt-1 flex items-center justify-between gap-2 border-t border-white/[0.07] pt-2.5">
            <button
              type="button"
              disabled={index === 0}
              onClick={() => setChapter(CHAPTERS[index - 1].id)}
              className="press glass-btn rounded-full px-3 py-1 text-[11px] text-ink disabled:opacity-30"
            >
              ← {index > 0 ? title(CHAPTERS[index - 1]) : "Previous"}
            </button>
            <button
              type="button"
              disabled={index === CHAPTERS.length - 1}
              onClick={() => setChapter(CHAPTERS[index + 1].id)}
              className="press glass-btn-accent rounded-full px-3 py-1 text-[11px] text-ink disabled:opacity-30"
            >
              {index < CHAPTERS.length - 1 ? title(CHAPTERS[index + 1]) : "Next"} →
            </button>
          </nav>
        </article>
      ) : (
        <ul className="stagger flex flex-col gap-1.5">
          {CHAPTERS.map((ch, i) => (
            <li key={ch.id}>
              <button
                type="button"
                onClick={() => setChapter(ch.id)}
                className="press glass flex w-full items-center gap-2.5 px-3 py-2 text-left hover:bg-white/[0.05]"
              >
                <span className="num grid h-6 w-6 shrink-0 place-items-center rounded-full bg-accent/15 text-[10.5px] font-semibold text-accent">
                  {i + 1}
                </span>
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="text-[12.5px] text-ink">{title(ch)}</span>
                  <span className="truncate text-[10.5px] text-muted">{ch.summary}</span>
                </span>
                <span className="-rotate-90 text-muted">
                  <ChevronIcon open={false} />
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}

      {zoomed && (
        <div
          role="dialog"
          aria-label={zoomed.alt}
          onClick={() => setZoomed(null)}
          className="reveal fixed inset-0 z-50 flex flex-col items-center justify-center gap-2 bg-black/80 p-3 backdrop-blur-sm"
        >
          <img src={zoomed.src} alt={zoomed.alt} className="max-h-[85%] max-w-full rounded-lg border border-white/15" />
          <span className="text-[11px] text-muted">{zoomed.alt} · tap or Esc to close</span>
        </div>
      )}
    </main>
  );
}
