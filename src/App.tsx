import { createContext, useCallback, useContext, useEffect, useLayoutEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Emblem } from "./components/Emblem";
import { BackupControls, ExpiryNotice, UpdatePanel } from "./components/Updates";
import { Logo } from "./components/Logo";
import { MicPicker } from "./components/MicPicker";
import { RangeSlider, Slider, formatDb } from "./components/Slider";
import { Switch } from "./components/Switch";
import { TestPanel } from "./components/TestPanel";
import { getMeters, meterPercent } from "./lib/monitor";
import {
  BackIcon,
  CableIcon,
  ChevronIcon,
  CloseIcon,
  ExpandIcon,
  GearIcon,
  MicIcon,
  MinusIcon,
} from "./components/icons";
import {
  type DeviceState,
  type Summary,
  VB_CABLE_URL,
  cableConflict,
  cableSummary,
  getDeviceState,
  micSummary,
  onDevicesChanged,
  refreshDevices,
  selectInputDevice,
  SHUUSSHY_MIC_LABEL,
  setVirtualMicName,
} from "./lib/devices";
import {
  type AppInfo,
  type Engine,
  type LicenseStatus,
  activateLicense,
  type ReleaseInfo,
  type UpdateInfo,
  exportSettings,
  getReleaseInfo,
  onUpdateAvailable,
  getAccentColor,
  getLicense,
  getStartWithWindows,
  openVbCableSite,
  setStartWithWindows,
  presetNeedsPro,
  removeLicense,
  type PresetState,
  type Settings,
  applyPreset,
  getAppInfo,
  getPresets,
  presetName,
  getSettings,
  inDesktopShell,
  onOpenSettings,
  onSettingsChanged,
  resetSettings,
  resizeBanner,
  updateSettings,
} from "./lib/ipc";
import {
  type RoutingStatus,
  engineDetail,
  getRoutingStatus,
  hasGlitches,
  isolationLine,
  strengthHelp,
  telemetryLine,
  onRoutingChanged,
  routingSummary,
} from "./lib/routing";
import { ENGINE_LOADING, errorText, presentStatus, type Tone } from "./lib/status";
import { ACCENTS, accentFilter, glassPointer } from "./lib/appearance";

type View = "main" | "settings";

/** True with a valid Shuusshy Pro key. Gating is enforced in the backend; this drives the UI. */
const ProContext = createContext(false);
const usePro = () => useContext(ProContext);

const toneDot: Record<Tone, string> = {
  neutral: "bg-muted",
  active: "bg-accent",
  warning: "bg-warn",
  error: "bg-danger",
};
const toneText: Record<Tone, string> = {
  neutral: "text-muted",
  active: "text-muted",
  warning: "text-warn",
  error: "text-danger",
};

interface DeviceControls {
  devices: DeviceState | null;
  busy: boolean;
  select: (id: string) => void;
  refresh: () => void;
  rename: (shuusshy: boolean) => void;
}

export default function App() {
  const desktop = inDesktopShell();
  const rootRef = useRef<HTMLDivElement>(null);
  const [view, setView] = useState<View>("main");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [devices, setDevices] = useState<DeviceState | null>(null);
  const [routing, setRouting] = useState<RoutingStatus | null>(null);
  const [presets, setPresets] = useState<PresetState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [testOpen, setTestOpen] = useState(false);
  const [license, setLicense] = useState<LicenseStatus | null>(null);
  const [focusLicense, setFocusLicense] = useState(false);
  const [release, setRelease] = useState<ReleaseInfo | null>(null);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [note, setNote] = useState<string | null>(null);

  useEffect(() => {
    if (!desktop) return;
    Promise.all([getSettings(), getAppInfo(), getDeviceState(), getLicense()])
      .then(([s, i, d, l]) => {
        setSettings(s);
        setInfo(i);
        setDevices(d);
        setLicense(l);
      })
      .catch((e) => setError(errorText(e)));
    // Hot-plug, selection, routing and tray changes are pushed from the backend;
    // routing statistics (buffer, drift, glitch counters) are polled once a second.
    const unlisten = [
      onDevicesChanged(setDevices),
      onRoutingChanged(setRouting),
      onSettingsChanged(setSettings),
      onUpdateAvailable(setUpdate),
      onOpenSettings(() => setView("settings")),
    ];
    // Release validity can change while the app runs (midnight): re-read it hourly.
    const readRelease = () => getReleaseInfo().then(setRelease).catch(() => {});
    readRelease();
    const releaseTimer = window.setInterval(readRelease, 3_600_000);
    const poll = () => getRoutingStatus().then(setRouting).catch(() => {});
    poll();
    const timer = window.setInterval(poll, 1000);
    return () => {
      window.clearInterval(timer);
      window.clearInterval(releaseTimer);
      unlisten.forEach((u) => u.then((f) => f()));
    };
  }, [desktop]);

  // Which preset (if any) the current settings correspond to.
  useEffect(() => {
    if (!desktop || !settings) return;
    getPresets().then(setPresets).catch(() => {});
  }, [desktop, settings]);

  // Size the native window to the banner's real content height.
  useLayoutEffect(() => {
    const el = rootRef.current;
    if (!el || !desktop) return;
    const ro = new ResizeObserver(() => {
      resizeBanner(Math.ceil(el.getBoundingClientRect().height)).catch((e) =>
        setError(errorText(e)),
      );
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [desktop]);

  const run = useCallback(async <T,>(op: () => Promise<T>, apply: (v: T) => void) => {
    setBusy(true);
    try {
      apply(await op());
      setError(null);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }, []);

  const deviceControls: DeviceControls = {
    devices,
    busy,
    select: (id) => run(() => selectInputDevice(id), setDevices),
    refresh: () => run(refreshDevices, setDevices),
    rename: (shuusshy) => run(() => setVirtualMicName(shuusshy), setDevices),
  };

  const status = presentStatus(routing?.engine ?? ENGINE_LOADING);
  const pro = license?.state === "pro";
  const openActivation = () => {
    setFocusLicense(true);
    setView("settings");
  };
  const save = (next: Settings) => run(() => updateSettings(next), setSettings);
  const compact = settings?.app.compact ?? false;
  const toTray = settings?.app.keepRunningInTray ?? false;
  const setCompact = (c: boolean) =>
    settings && save({ ...settings, app: { ...settings.app, compact: c } });

  // Appearance: see-through glass and accent colour (whole UI, emblem; tray is done in Rust).
  const glass = settings?.app.glass ?? false;
  const tint = settings?.app.glassTint ?? 0.6;
  const accent = settings?.app.accent;
  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("glass-on", glass);
    root.style.setProperty("--tint", String(tint));
  }, [glass, tint]);
  useEffect(() => {
    if (!desktop || !accent) return;
    getAccentColor()
      .then((hex) => {
        const root = document.documentElement;
        const f = accentFilter(hex);
        root.style.setProperty("--color-accent", hex);
        root.style.setProperty("--accent-shift", `${f.shiftDeg}deg`);
        root.style.setProperty("--accent-sat", String(f.saturate));
      })
      .catch(() => {});
  }, [desktop, accent]);

  return (
    // Max height matches the backend's BANNER_MAX_HEIGHT; taller content scrolls inside.
    <ProContext.Provider value={pro}>
    <div
      ref={rootRef}
      onPointerMove={glassPointer}
      className="app-shell @container flex max-h-[640px] flex-col"
    >
      <div className="ambient" aria-hidden="true">
        <span />
        <span />
        <span />
      </div>
      <header
        data-tauri-drag-region
        className="relative flex h-11 items-center gap-2 border-b border-white/[0.07] bg-white/[0.03] px-3 backdrop-blur-xl"
      >
        {/* Accent hairline under the header while noise cancellation is really working. */}
        <span
          aria-hidden="true"
          className={`pointer-events-none absolute inset-x-0 -bottom-px h-px bg-gradient-to-r from-transparent via-accent to-transparent
            transition-opacity duration-500 ${status.tone === "active" ? "opacity-80" : "opacity-0"}`}
        />
        {view === "settings" ? (
          <IconButton label="Back" onClick={() => setView("main")}>
            <BackIcon />
          </IconButton>
        ) : (
          <Logo />
        )}
        <span className="font-display pointer-events-none text-[15px] font-semibold tracking-tight">
          {view === "settings" ? "Settings" : "Shuusshy"}
        </span>
        {view === "main" && (
          <span
            className={`pointer-events-none ml-1 flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[10.5px] font-medium transition-colors duration-300
              ${status.tone === "active" ? "glass-btn-accent text-ink" : "well text-muted"}`}
            title={status.detail}
            role="status"
            aria-live="polite"
          >
            <span
              className={`h-1.5 w-1.5 rounded-full transition-colors duration-300 ${toneDot[status.tone]}
                ${status.tone === "active" ? "dot-live" : ""}`}
            />
            {status.label}
          </span>
        )}
        <div data-tauri-drag-region className="flex-1 self-stretch" />
        {view === "main" && (
          <>
            <IconButton
              label={compact ? "Expand banner" : "Compact banner"}
              disabled={!settings}
              onClick={() => setCompact(!compact)}
            >
              <ExpandIcon expanded={!compact} />
            </IconButton>
            <IconButton label="Settings" onClick={() => setView("settings")}>
              <GearIcon />
            </IconButton>
          </>
        )}
        <IconButton
          label="Minimize"
          disabled={!desktop}
          onClick={() => getCurrentWindow().minimize()}
        >
          <MinusIcon />
        </IconButton>
        <IconButton
          label={toTray ? "Close to tray (keeps processing)" : "Exit Shuusshy"}
          disabled={!desktop}
          onClick={() => getCurrentWindow().close()}
          danger
        >
          <CloseIcon />
        </IconButton>
      </header>

      {!desktop && (
        <Notice tone="warning">
          Running in a browser preview. Desktop features are unavailable.
        </Notice>
      )}
      {error && (
        <Notice tone="error" onDismiss={() => setError(null)}>
          {error}
        </Notice>
      )}
      {note && (
        <Notice tone="info" onDismiss={() => setNote(null)}>
          {note}
        </Notice>
      )}
      {devices?.error && <Notice tone="error">Audio devices: {devices.error}</Notice>}
      {info && !info.settingsPersistent && (
        <Notice tone="warning">Settings storage unavailable — changes won't be saved.</Notice>
      )}

      {view === "main" && (
        <div className="px-3 pt-2 empty:hidden">
          <ExpiryNotice
            release={release}
            onExport={() => run(exportSettings, (p) => setNote(`Settings saved to ${p}`))}
          />
          {update && release?.validity !== "expired" && (
            <button
              type="button"
              onClick={() => setView("settings")}
              className="press glass-btn-accent reveal mt-1 rounded-full px-3 py-1 text-[11px]"
            >
              Shuusshy {update.version} is available{update.ready ? ", ready to install" : ""} →
            </button>
          )}
        </div>
      )}
      {view === "main" ? (
        <MainView
          dc={deviceControls}
          routing={routing}
          settings={settings}
          preset={presetName(presets)}
          compact={compact}
          onChange={save}
          testOpen={testOpen}
          setTestOpen={setTestOpen}
          onActivate={openActivation}
        />
      ) : (
        <SettingsView
          key="settings"
          onOpenTest={() => {
            setTestOpen(true);
            setView("main");
          }}
          settings={settings}
          info={info}
          dc={deviceControls}
          routing={routing}
          presets={presets}
          onApplyPreset={(id) => run(() => applyPreset(id), setSettings)}
          onChange={save}
          onReset={() =>
            run(resetSettings, (s) => {
              setSettings(s);
            })
          }
          license={license}
          focusLicense={focusLicense}
          onLicenseFocused={() => setFocusLicense(false)}
          onActivate={(key) => run(() => activateLicense(key), setLicense)}
          onRemoveLicense={() => run(removeLicense, setLicense)}
          release={release}
          update={update}
          onImported={setSettings}
        />
      )}
    </div>
    </ProContext.Provider>
  );
}

function MainView({
  dc,
  routing,
  settings,
  preset,
  compact,
  onChange,
  testOpen,
  setTestOpen,
  onActivate,
}: {
  dc: DeviceControls;
  routing: RoutingStatus | null;
  settings: Settings | null;
  preset: string | null;
  compact: boolean;
  onChange: (s: Settings) => void;
  testOpen: boolean;
  setTestOpen: (update: (open: boolean) => boolean) => void;
  onActivate: () => void;
}) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const pro = usePro();
  const showActivation = !pro && settings && !settings.app.activationPromptDismissed && !compact;
  const mic: Summary | null = dc.devices ? micSummary(dc.devices.mic) : null;
  const route: Summary | null = routing ? routingSummary(routing) : null;
  const conflict = dc.devices ? cableConflict(dc.devices.cableUsers) : null;

  return (
    <main className="enter-left flex flex-col gap-2 px-3 py-3">
      <div className="glass flex flex-col">
        <button
          type="button"
          onClick={() => setPickerOpen((o) => !o)}
          disabled={!dc.devices}
          aria-expanded={pickerOpen}
          className="press flex items-center gap-2 rounded-[17px] px-3 py-2 text-left hover:bg-white/[0.05] disabled:opacity-50"
          title={mic ? `${mic.title} — ${mic.detail}` : undefined}
        >
          <span className="text-muted">
            <MicIcon />
          </span>
          <span className="flex min-w-0 flex-1 flex-col">
            <span className="truncate text-[13px]">{mic?.title ?? "Loading devices…"}</span>
            {mic && (
              <span className={`truncate text-[11px] ${toneText[mic.tone]}`}>{mic.detail}</span>
            )}
          </span>
          {mic && <span className={`h-2 w-2 shrink-0 rounded-full ${toneDot[mic.tone]}`} />}
          <span className="text-muted">
            <ChevronIcon open={pickerOpen} />
          </span>
        </button>
        {pickerOpen && dc.devices && (
          <div className="reveal border-t border-line/60 px-1 py-1.5">
            <MicPicker
              state={dc.devices}
              busy={dc.busy}
              onSelect={(id) => {
                dc.select(id);
                setPickerOpen(false);
              }}
              onRefresh={dc.refresh}
            />
          </div>
        )}
      </div>

      {/* Compact mode shows the virtual-mic line only when it needs attention. */}
      {route && (!compact || route.tone === "error" || route.tone === "warning") && (
        <div
          className="flex items-center gap-2 px-1 text-[11px] text-muted"
          title={`${route.title} — ${route.detail}`}
        >
          <CableIcon />
          <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${toneDot[route.tone]}`} />
          <span className={`truncate ${route.tone === "error" ? "text-danger" : ""}`}>
            <span className="text-ink/90">{route.title}</span> · {route.detail}
          </span>
        </div>
      )}

      {conflict && (
        <div className="reveal rounded-lg border border-warn/40 bg-warn/10 px-2.5 py-1.5 text-[11px] leading-snug text-warn">
          <span className="font-semibold">{conflict.title}.</span> {conflict.detail}
        </div>
      )}

      <NoiseCard
        routing={routing}
        settings={settings}
        preset={preset}
        compact={compact}
        onChange={onChange}
        testOpen={testOpen}
        onTest={() => setTestOpen((o) => !o)}
      />
      {testOpen && <TestPanel routing={routing} onClose={() => setTestOpen(() => false)} />}
      {showActivation && (
        <ActivationCard
          onActivate={onActivate}
          onDismiss={() =>
            onChange({ ...settings, app: { ...settings.app, activationPromptDismissed: true } })
          }
        />
      )}
    </main>
  );
}

function NoiseCard({
  routing,
  settings,
  preset,
  compact,
  onChange,
  testOpen,
  onTest,
}: {
  routing: RoutingStatus | null;
  settings: Settings | null;
  preset: string | null;
  compact: boolean;
  onChange: (s: Settings) => void;
  testOpen: boolean;
  onTest: () => void;
}) {
  const on = settings?.audio.suppressionEnabled ?? false;
  const strength = settings?.audio.suppressionStrength ?? 0;
  // The emblem and glow follow the engine's real state, not the switch.
  const live = routing?.engine.kind === "processing";
  const scans = useRef(0);
  const wasLive = useRef(live);
  if (live && !wasLive.current) scans.current += 1;
  wasLive.current = live;
  const detail = routing ? engineDetail(routing, strength, preset) : "Starting…";
  const setAudio = (patch: Partial<Settings["audio"]>) =>
    settings && onChange({ ...settings, audio: { ...settings.audio, ...patch } });
  return (
    <div className={`glass flex flex-col gap-2 px-3 py-2.5 ${live ? "glass-live" : ""}`}>
      {scans.current > 0 && <span key={scans.current} className="scan-beam" aria-hidden="true" />}
      <div className="flex items-center gap-2.5">
        <Emblem live={live} size={compact ? 40 : 48} />
        <div className="flex min-w-0 flex-1 flex-col">
          <span className={`label-tech transition-colors duration-300 ${live ? "text-accent" : "text-ink/90"}`}>
            Noise cancellation
          </span>
          <span className="truncate text-[11px] text-muted" title={detail}>
            {detail}
          </span>
        </div>
        <Switch
          checked={on}
          disabled={!settings}
          label="Noise cancellation"
          onChange={(v) => setAudio({ suppressionEnabled: v })}
        />
        <button
          type="button"
          onClick={onTest}
          aria-expanded={testOpen}
          title="Hear and measure your microphone before and after noise suppression"
          className={`press label-tech rounded-full px-3 py-1.5 text-ink
            ${testOpen ? "glass-btn-accent" : "glass-btn"}`}
        >
          Test
        </button>
      </div>
      {!compact && routing?.state === "running" && <ActivityBar />}
      {!compact && settings && (
        <EngineToggle engine={settings.audio.engine} onChange={(e) => setAudio({ engine: e })} />
      )}
      {!compact && settings && (
        <div className="flex items-center gap-2 pl-1">
          <span className="label-tech w-16 !text-[9.5px] text-muted">Strength</span>
          <div className="flex-1">
            <Slider
              label="Noise suppression strength"
              value={strength}
              disabled={!on}
              onCommit={(v) => setAudio({ suppressionStrength: v })}
            />
          </div>
        </div>
      )}
    </div>
  );
}

const ENGINES = [
  { id: "rnnoise", name: "RNNoise", note: "Lightweight · 10 ms delay" },
  { id: "deepFilterNet", name: "DeepFilterNet", note: "Strongest · 30 ms delay" },
] as const;

/** Two-way engine selector for the expanded banner. */
function EngineToggle({ engine: stored, onChange }: { engine: Engine; onChange: (e: Engine) => void }) {
  const pro = usePro();
  // Without Pro, RNNoise is what really runs, so that is what the toggle shows.
  const engine: Engine = pro ? stored : "rnnoise";
  return (
    <div className="flex items-center gap-2 pl-1">
      <span className="label-tech w-16 !text-[9.5px] text-muted">Engine</span>
      <div role="radiogroup" aria-label="Engine" className="well relative flex flex-1 rounded-full p-[3px]">
        {/* Sliding highlight behind the selected engine. */}
        <span
          aria-hidden="true"
          className="liquid-lens absolute inset-y-[3px] left-[3px] w-[calc(50%-3px)] rounded-full"
          style={{ transform: engine === "deepFilterNet" ? "translateX(100%)" : "none" }}
        />
        {ENGINES.map((e) => {
          const selected = engine === e.id;
          const locked = e.id === "deepFilterNet" && !pro;
          return (
            <button
              key={e.id}
              type="button"
              role="radio"
              aria-checked={selected}
              disabled={locked}
              title={locked ? "DeepFilterNet is part of Shuusshy Pro" : e.note}
              onClick={() => !selected && onChange(e.id)}
              className={`press relative z-10 flex-1 rounded-full px-2 py-1 text-[11px] transition-colors duration-200
                ${selected ? "text-ink" : "text-muted hover:text-ink"}`}
            >
              {e.name}
              {locked && <ProBadge />}
            </button>
          );
        })}
      </div>
    </div>
  );
}

function ProBadge() {
  return (
    <span className="label-tech ml-1 rounded border border-accent-2/50 bg-accent-2/15 px-1 !text-[8.5px] !tracking-[0.1em] text-accent-2">
      Pro
    </span>
  );
}

/** First-run offer to activate Pro (spec: key entered on or after installation). */
function ActivationCard({ onActivate, onDismiss }: { onActivate: () => void; onDismiss: () => void }) {
  return (
    <div className="glass reveal flex items-center gap-3 px-3 py-2.5">
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="label-tech flex items-center gap-1.5 text-accent-2">
          Shuusshy Pro <ProBadge />
        </span>
        <span className="text-[11px] leading-snug text-muted">
          Have a key? Unlock DeepFilterNet and voice isolation. RNNoise stays free.
        </span>
      </div>
      <div className="flex flex-col items-end gap-1">
        <button
          type="button"
          onClick={onActivate}
          className="press rounded-lg border border-accent-2/60 bg-accent-2/15 px-2.5 py-1 text-[11.5px] text-ink hover:bg-accent-2/25"
        >
          Enter key
        </button>
        <button type="button" onClick={onDismiss} className="press text-[10.5px] text-muted hover:text-ink">
          Not now
        </button>
      </div>
    </div>
  );
}

/** Thin live activity indicator for the banner: the cleaned output's real level. */
function ActivityBar() {
  const [db, setDb] = useState<number | null>(null);
  useEffect(() => {
    let alive = true;
    const tick = () =>
      getMeters()
        .then((m) => alive && setDb(m ? m.outputRmsDb : null))
        .catch(() => {});
    const t = window.setInterval(tick, 100);
    tick();
    return () => {
      alive = false;
      window.clearInterval(t);
    };
  }, []);
  return (
    <div
      className="well h-1.5 overflow-hidden rounded-full"
      title="Output level (what apps hear)"
      role="meter"
      aria-label="Output level"
      aria-valuemin={-60}
      aria-valuemax={0}
      aria-valuenow={db ?? -60}
    >
      <div
        className="h-full rounded-full bg-gradient-to-r from-accent-dim to-accent shadow-[0_0_8px] shadow-accent/60 transition-[width] duration-100"
        style={{ width: `${db === null ? 0 : meterPercent(db)}%` }}
      />
    </div>
  );
}

function SettingsView({
  settings,
  info,
  dc,
  routing,
  presets,
  onApplyPreset,
  onChange,
  onReset,
  onOpenTest,
  license,
  focusLicense,
  onLicenseFocused,
  onActivate,
  onRemoveLicense,
  release,
  update,
  onImported,
}: {
  release: ReleaseInfo | null;
  update: UpdateInfo | null;
  onImported: (s: Settings) => void;
  license: LicenseStatus | null;
  focusLicense: boolean;
  onLicenseFocused: () => void;
  onActivate: (key: string) => void;
  onRemoveLicense: () => void;
  settings: Settings | null;
  info: AppInfo | null;
  dc: DeviceControls;
  routing: RoutingStatus | null;
  presets: PresetState | null;
  onApplyPreset: (id: string) => void;
  onChange: (s: Settings) => void;
  onReset: () => void;
  onOpenTest: () => void;
}) {
  const cable = dc.devices ? cableSummary(dc.devices.cable) : null;
  const conflict = dc.devices ? cableConflict(dc.devices.cableUsers) : null;
  const setAudio = (patch: Partial<Settings["audio"]>) =>
    settings && onChange({ ...settings, audio: { ...settings.audio, ...patch } });
  const setApp = (patch: Partial<Settings["app"]>) =>
    settings && onChange({ ...settings, app: { ...settings.app, ...patch } });
  const limited = (routing?.chainTelemetry?.limitedFrames ?? 0) > 0;
  return (
    <main className="enter-right stagger scroll-thin min-h-0 gap-3 overflow-y-auto px-3 py-3 text-[13px] @min-[500px]:columns-2 [&>*]:mb-3 [&>*]:break-inside-avoid">
      <Section title="Microphone">
        {dc.devices ? (
          <div className="-mx-2">
            <MicPicker
              state={dc.devices}
              busy={dc.busy}
              onSelect={dc.select}
              onRefresh={dc.refresh}
            />
          </div>
        ) : (
          <span className="text-[12px] text-muted">Loading devices…</span>
        )}
        {settings && (
          <GainControl
            label="Input gain"
            value={settings.audio.inputGainDb}
            help="Applied before noise suppression; raise it if your microphone is quiet."
            onCommit={(v) => setAudio({ inputGainDb: v })}
          />
        )}
      </Section>

      <Section title="Noise Suppression">
        <PresetChips presets={presets} onApply={onApplyPreset} />
        <NoiseSettings settings={settings} routing={routing} onChange={onChange} />
        {settings && (
          <Disclosure label="Advanced: noise gate">
            <GateSettings settings={settings} routing={routing} onChange={onChange} />
          </Disclosure>
        )}
      </Section>

      <Section title="Audio Output">
        {cable && (
          <div className="flex flex-col gap-1">
            <span className="flex items-center gap-2">
              <span className={`h-2 w-2 rounded-full ${toneDot[cable.tone]}`} />
              {cable.title}
            </span>
            <span className={`text-[11px] leading-snug ${toneText[cable.tone]}`}>
              {cable.detail}
            </span>
          </div>
        )}
        {dc.devices && dc.devices.cable.state !== "ready" && <CableSetupSteps />}
        {conflict && (
          <p className="text-[11px] leading-snug text-warn">
            <span className="font-semibold">{conflict.title}.</span> {conflict.detail}
          </p>
        )}
        {dc.devices?.cable.state === "ready" && (
          <MicNameControl
            current={dc.devices.cable.capture.description ?? dc.devices.cable.capture.name}
            onRename={dc.rename}
            busy={dc.busy}
          />
        )}
        <Row label="Send microphone to virtual mic">
          <Switch
            size="sm"
            label="Send microphone to virtual mic"
            checked={settings?.audio.routingEnabled ?? false}
            disabled={!settings}
            onChange={(v) => setAudio({ routingEnabled: v })}
          />
        </Row>
        {settings && (
          <GainControl
            label="Output gain"
            value={settings.audio.outputGainDb}
            help={
              limited
                ? "The safety limiter is reducing peaks — lower the output gain."
                : "Level that apps receive. A safety limiter prevents clipping."
            }
            warn={limited}
            onCommit={(v) => setAudio({ outputGainDb: v })}
          />
        )}
        {routing && (
          <Disclosure label="Connection details">
            <RoutingDetails routing={routing} />
          </Disclosure>
        )}
      </Section>

      <Section title="Testing and Monitoring">
        <p className="text-[11px] leading-snug text-muted">
          Hear your microphone before and after noise suppression on your headphones, with live
          level meters. Nothing is recorded or saved.
        </p>
        <button
          type="button"
          onClick={onOpenTest}
          className="press glass-btn-accent self-start rounded-full px-3.5 py-1.5 text-[12px] font-medium text-ink"
        >
          Open live test
        </button>
      </Section>

      <Section title="Appearance">
        {settings && <AppearanceSettings settings={settings} onChange={onChange} />}
      </Section>

      <Section title="Startup and Behavior">
        <StartWithWindows portable={info?.portable ?? false} />
        <Row label="Keep banner on top">
          <Switch
            size="sm"
            label="Keep banner on top"
            checked={settings?.app.alwaysOnTop ?? false}
            disabled={!settings}
            onChange={(v) => setApp({ alwaysOnTop: v })}
          />
        </Row>
        <Row label="Compact banner">
          <Switch
            size="sm"
            label="Compact banner"
            checked={settings?.app.compact ?? false}
            disabled={!settings}
            onChange={(v) => setApp({ compact: v })}
          />
        </Row>
        <Row label="Keep running in the tray when closed">
          <Switch
            size="sm"
            label="Keep running in the tray when closed"
            checked={settings?.app.keepRunningInTray ?? false}
            disabled={!settings}
            onChange={(v) => setApp({ keepRunningInTray: v })}
          />
        </Row>
        <span className="text-[10.5px] leading-snug text-muted">
          {settings?.app.keepRunningInTray
            ? "Closing the banner hides it; your virtual microphone keeps working. Use the tray icon to bring it back or to exit."
            : "Closing the banner exits Shuusshy and the virtual microphone goes silent."}
        </span>
      </Section>

      <Section title="Application and Updates">
        <LicenseControl
          license={license}
          focus={focusLicense}
          onFocused={onLicenseFocused}
          onActivate={onActivate}
          onRemove={onRemoveLicense}
          busy={dc.busy}
        />
        <Row label="Version">
          <span className="flex items-center gap-1.5 text-muted">
            {info?.portable && (
              <span
                className="well rounded-full px-2 py-0.5 text-[10px] font-semibold text-ink/80"
                title="Running from the portable ZIP: settings are stored in the data folder next to shuusshy.exe"
              >
                Portable
              </span>
            )}
            {info?.version ?? "—"}
          </span>
        </Row>
        <UpdatePanel release={release} settings={settings} onChange={onChange} announced={update} />
        <BackupControls onImported={onImported} />
        {info && (
          <p className="select-text break-all text-[11px] leading-snug text-muted/70" title="Settings folder">
            Settings: {info.dataDir}
          </p>
        )}
        <button
          type="button"
          onClick={onReset}
          disabled={!settings}
          className="press self-start rounded-lg px-2 py-1 text-[12px] text-muted hover:bg-panel-hi hover:text-danger disabled:opacity-40"
        >
          Reset settings to defaults
        </button>
      </Section>

      <Section title="About Shuusshy">
        <AboutCard version={info?.version ?? null} />
      </Section>
    </main>
  );
}

/** App identity, creator credit, what Shuusshy does and the work it builds on. */
function AboutCard({ version }: { version: string | null }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-3">
        <Emblem live size={52} />
        <div className="flex min-w-0 flex-col">
          <span className="font-display text-[17px] font-semibold tracking-tight">Shuusshy</span>
          <span className="text-[11.5px] text-accent">Less Noise. More Voice.</span>
          <span className="num text-[10.5px] text-muted">Version {version ?? "—"}</span>
        </div>
      </div>

      <p className="text-[11px] leading-snug text-muted">
        Shuusshy removes background noise from your microphone and passes your clean voice to
        Teams, Discord, Zoom and other apps through a virtual microphone. All processing happens
        on this PC: no account, no audio recorded or uploaded. The only network use is the optional
        update check (release information from GitHub).
      </p>

      <div className="flex flex-col gap-1">
        <span className="label-tech text-ink/75">Built with</span>
        <ul className="flex flex-col gap-0.5 text-[10.5px] leading-snug text-muted">
          <li>RNNoise (via nnnoiseless) and DeepFilterNet for noise suppression</li>
          <li>VB-CABLE by VB-Audio for the virtual microphone (installed separately)</li>
          <li>Tauri, React and Rust; emblem and icons rendered in Blender</li>
        </ul>
        <span className="text-[10.5px] text-muted/80">
          Full licences: THIRD_PARTY_LICENSES.md, shipped with Shuusshy.
        </span>
      </div>

      <span className="text-[10px] text-muted/70">Created by Ian Musa · © 2026</span>
    </div>
  );
}

/** Glass window, transparency and accent colour. */
function AppearanceSettings({
  settings,
  onChange,
}: {
  settings: Settings;
  onChange: (s: Settings) => void;
}) {
  const app = settings.app;
  const setApp = (patch: Partial<Settings["app"]>) =>
    onChange({ ...settings, app: { ...app, ...patch } });
  return (
    <div className="flex flex-col gap-2.5">
      <Row label="Glass window">
        <Switch size="sm" label="Glass window" checked={app.glass} onChange={(v) => setApp({ glass: v })} />
      </Row>
      <div className="flex flex-col gap-1">
        <span className="text-[12px]">Transparency</span>
        <RangeSlider
          label="Transparency"
          value={Math.round((1 - app.glassTint) * 100)}
          min={5}
          max={65}
          step={1}
          format={(v) => `${v}%`}
          disabled={!app.glass}
          onCommit={(v) => setApp({ glassTint: +(1 - v / 100).toFixed(2) })}
        />
        <span className="text-[10.5px] leading-snug text-muted">
          With glass on, your desktop shows blurred behind the banner. Lower the transparency if
          text is hard to read over bright wallpapers.
        </span>
      </div>
      <div className="flex flex-col gap-1.5">
        <span className="text-[12px]">Accent colour</span>
        <div role="radiogroup" aria-label="Accent colour" className="flex flex-wrap gap-2">
          {ACCENTS.map((a) => {
            const selected = app.accent === a.id;
            return (
              <button
                key={a.id}
                type="button"
                role="radio"
                aria-checked={selected}
                aria-label={a.name}
                title={a.name}
                onClick={() => setApp({ accent: a.id })}
                className={`press relative grid h-7 w-7 place-items-center rounded-full
                  ${selected ? "ring-2 ring-white/80 ring-offset-2 ring-offset-transparent" : "ring-1 ring-white/15 hover:ring-white/40"}`}
                style={{
                  background: a.hex
                    ? `radial-gradient(120% 120% at 30% 25%, #fff8, transparent 45%), ${a.hex}`
                    : "conic-gradient(from 0deg, #39d5ff, #9b8cff, #ff6b9d, #ffb547, #4de3b0, #39d5ff)",
                  boxShadow: selected && a.hex ? `0 0 14px -2px ${a.hex}` : undefined,
                }}
              >
                {selected && <span className="h-2 w-2 rounded-full bg-white shadow" />}
              </button>
            );
          })}
        </div>
        <span className="text-[10.5px] text-muted">
          {ACCENTS.find((a) => a.id === app.accent)?.name}. Colours the highlights, the emblem and
          the tray icon.
        </span>
      </div>
    </div>
  );
}

/** Pro key entry and status. Keys are checked offline on this PC. */
function LicenseControl({
  license,
  focus,
  onFocused,
  onActivate,
  onRemove,
  busy,
}: {
  license: LicenseStatus | null;
  focus: boolean;
  onFocused: () => void;
  onActivate: (key: string) => void;
  onRemove: () => void;
  busy: boolean;
}) {
  const [key, setKey] = useState("");
  const ref = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (!focus) return;
    ref.current?.scrollIntoView({ behavior: "smooth", block: "center" });
    input.current?.focus();
    onFocused();
  }, [focus, onFocused]);
  if (!license) return null;
  if (license.state === "pro") {
    return (
      <div ref={ref} className="flex flex-col gap-1 rounded-lg border border-accent-2/40 bg-accent-2/10 px-2.5 py-2">
        <span className="label-tech flex items-center gap-1.5 text-accent-2">
          Shuusshy Pro active <ProBadge />
        </span>
        <span className="num text-[11px] text-muted">
          Licence #{license.serial} · issued {license.issued}
          {license.expires ? ` · valid until ${license.expires}` : " · no expiry"}
        </span>
        <button
          type="button"
          onClick={onRemove}
          disabled={busy}
          className="press self-start text-[10.5px] text-muted hover:text-danger disabled:opacity-40"
        >
          Remove key from this PC
        </button>
      </div>
    );
  }
  return (
    <div ref={ref} className="flex flex-col gap-1.5 rounded-lg border border-line bg-base/50 px-2.5 py-2">
      <span className="label-tech flex items-center gap-1.5 text-ink/90">
        Shuusshy Pro <ProBadge />
      </span>
      {license.state === "expired" && (
        <span className="text-[11px] text-warn">Your key #{license.serial} expired on {license.expires}.</span>
      )}
      <span className="text-[10.5px] leading-snug text-muted">
        Unlocks DeepFilterNet, voice isolation and the presets built on them. Checked offline on this
        PC: no account, no internet.
      </span>
      <label htmlFor="license-key" className="sr-only">
        License key
      </label>
      <textarea
        id="license-key"
        ref={input}
        value={key}
        rows={2}
        spellCheck={false}
        placeholder="SHUU-XXXXX-XXXXX-…"
        onChange={(e) => setKey(e.target.value)}
        className="num resize-none rounded-lg border border-line bg-base px-2 py-1.5 text-[11px] text-ink outline-none select-text focus:border-accent-2/70"
      />
      <button
        type="button"
        disabled={busy || key.trim().length < 10}
        onClick={() => onActivate(key)}
        className="press self-start rounded-lg border border-accent-2/60 bg-accent-2/15 px-3 py-1 text-[12px] text-ink hover:bg-accent-2/25 disabled:opacity-40"
      >
        Activate
      </button>
    </div>
  );
}

/** How the virtual mic is named in Teams, Discord, Zoom… and the switch to Shuusshy's name. */
function MicNameControl({
  current,
  onRename,
  busy,
}: {
  current: string;
  onRename: (shuusshy: boolean) => void;
  busy: boolean;
}) {
  const named = current === SHUUSSHY_MIC_LABEL;
  return (
    <div className="flex flex-col gap-1.5 rounded-lg border border-line bg-base/50 px-2.5 py-2">
      <span className="text-[11px] text-muted">In Teams, Discord, Zoom… choose</span>
      <span className={`font-display text-[13px] ${named ? "text-accent" : "text-ink"}`}>
        {current} <span className="text-muted">(VB-Audio Virtual Cable)</span>
      </span>
      <button
        type="button"
        disabled={busy}
        onClick={() => onRename(!named)}
        className={`press self-start rounded-lg border px-2.5 py-1 text-[11.5px] disabled:opacity-40
          ${named ? "border-line text-muted hover:text-ink" : "border-accent/50 bg-accent/10 text-ink hover:bg-accent/20"}`}
      >
        {named ? "Restore the name “CABLE Output”" : `Show as “${SHUUSSHY_MIC_LABEL}”`}
      </button>
      <span className="text-[10.5px] leading-snug text-muted">
        Changes only the name Windows shows for VB-CABLE's microphone. Windows asks for
        administrator permission. Apps may need a restart to show the new name; a VB-CABLE
        update can reset it.
      </span>
    </div>
  );
}

function GainControl({
  label,
  value,
  help,
  warn,
  onCommit,
}: {
  label: string;
  value: number;
  help: string;
  warn?: boolean;
  onCommit: (v: number) => void;
}) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[12px]">{label}</span>
      <RangeSlider
        label={label}
        value={value}
        min={-24}
        max={24}
        step={0.5}
        format={formatDb}
        onCommit={onCommit}
      />
      <span className={`text-[10.5px] leading-snug ${warn ? "text-warn" : "text-muted"}`}>{help}</span>
    </div>
  );
}

function GateSettings({
  settings,
  routing,
  onChange,
}: {
  settings: Settings;
  routing: RoutingStatus | null;
  onChange: (s: Settings) => void;
}) {
  const g = settings.audio.noiseGate;
  const setGate = (patch: Partial<Settings["audio"]["noiseGate"]>) =>
    onChange({ ...settings, audio: { ...settings.audio, noiseGate: { ...g, ...patch } } });
  const c = routing?.chainTelemetry;
  return (
    <div className="flex flex-col gap-2">
      <Row label="Noise gate">
        <Switch size="sm" label="Noise gate" checked={g.enabled} onChange={(v) => setGate({ enabled: v })} />
      </Row>
      <div className="flex flex-col gap-1">
        <span className="text-[12px]">Gate threshold</span>
        <RangeSlider
          label="Noise gate threshold"
          value={g.thresholdDb}
          min={-90}
          max={0}
          step={1}
          format={(v) => `${v.toFixed(0)} dB`}
          disabled={!g.enabled}
          onCommit={(v) => setGate({ thresholdDb: v })}
        />
        <span className="text-[10.5px] leading-snug text-muted">
          Mutes the output while it stays below the threshold (after noise suppression), with a
          short hold so pauses between words are not cut.
          {c && g.enabled && (
            <span className={c.gateOpen ? "text-accent" : "text-warn"}>
              {" "}
              Gate is {c.gateOpen ? "open" : "closed"} now.
            </span>
          )}
        </span>
      </div>
    </div>
  );
}

/** Optional detail that stays folded away until asked for (spec 6.6: advanced is optional). */
function Disclosure({ label, children }: { label: string; children: React.ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="flex flex-col gap-2 border-t border-line/50 pt-2">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className="flex items-center justify-between rounded text-left text-[12px] text-muted hover:text-ink"
      >
        {label}
        <ChevronIcon open={open} />
      </button>
      {open && <div className="reveal">{children}</div>}
    </div>
  );
}

function PresetChips({
  presets,
  onApply,
}: {
  presets: PresetState | null;
  onApply: (id: string) => void;
}) {
  const pro = usePro();
  if (!presets) return null;
  const custom = presets.current === "custom";
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[12px]">Preset</span>
      <div role="radiogroup" aria-label="Preset" className="flex flex-wrap gap-1.5">
        {presets.presets.map((p) => {
          const selected = p.id === presets.current;
          const locked = presetNeedsPro(p) && !pro;
          return (
            <button
              key={p.id}
              type="button"
              role="radio"
              aria-checked={selected}
              disabled={locked}
              title={locked ? `${p.description} (Shuusshy Pro)` : p.description}
              onClick={() => onApply(p.id)}
              className={`press rounded-full border px-2.5 py-1 text-[11px]
                ${selected ? "border-accent/70 bg-accent/15 text-ink shadow-[0_0_12px_-5px] shadow-accent" : "border-line text-muted hover:border-accent/40 hover:bg-panel-hi hover:text-ink"}`}
            >
              {p.name}
              {locked && <ProBadge />}
            </button>
          );
        })}
        <span
          className={`rounded-full border px-2.5 py-1 text-[11px] ${custom ? "border-accent bg-accent/15 text-ink" : "border-transparent text-muted/60"}`}
        >
          Custom
        </span>
      </div>
      <span className="text-[10.5px] leading-snug text-muted">
        {custom
          ? "Custom: your own combination of engine, strength and gate."
          : presets.presets.find((p) => p.id === presets.current)?.description}
      </span>
    </div>
  );
}

function NoiseSettings({
  settings,
  routing,
  onChange,
}: {
  settings: Settings | null;
  routing: RoutingStatus | null;
  onChange: (s: Settings) => void;
}) {
  if (!settings) return <span className="text-[12px] text-muted">Loading…</span>;
  const a = settings.audio;
  const setAudio = (patch: Partial<Settings["audio"]>) =>
    onChange({ ...settings, audio: { ...a, ...patch } });
  const t = routing?.engineTelemetry;
  const st = routing ? presentStatus(routing.engine) : null;
  const pro = usePro();
  const engineShown: Engine = pro ? a.engine : "rnnoise";
  return (
    <div className="flex flex-col gap-2">
      <Row label="Noise cancellation">
        <Switch
          size="sm"
          label="Noise cancellation"
          checked={a.suppressionEnabled}
          onChange={(v) => setAudio({ suppressionEnabled: v })}
        />
      </Row>
      <div role="radiogroup" aria-label="Engine" className="flex gap-1.5">
        {ENGINES.map((e) => {
          const selected = engineShown === e.id;
          const locked = e.id === "deepFilterNet" && !pro;
          return (
            <button
              key={e.id}
              type="button"
              role="radio"
              aria-checked={selected}
              disabled={locked}
              onClick={() => setAudio({ engine: e.id })}
              className={`press flex flex-1 flex-col rounded-lg border px-2 py-1.5 text-left
                ${selected ? "border-accent/70 bg-accent/10 shadow-[0_0_14px_-6px] shadow-accent" : "border-line hover:border-accent/40 hover:bg-panel-hi"}`}
            >
              <span className="flex items-center text-[12px]">
                {e.name}
                {locked && <ProBadge />}
              </span>
              <span className="text-[10.5px] text-muted">{e.note}</span>
            </button>
          );
        })}
      </div>
      {!pro && a.engine === "deepFilterNet" && (
        <span className="text-[10.5px] leading-snug text-warn">
          DeepFilterNet needs Shuusshy Pro, so RNNoise is running. Your choice is kept for when you
          activate a key.
        </span>
      )}
      <div className="flex flex-col gap-1">
        <span className="text-[12px]">Strength</span>
        <Slider
          label="Noise suppression strength"
          value={a.suppressionStrength}
          disabled={!a.suppressionEnabled}
          onCommit={(v) => setAudio({ suppressionStrength: v })}
        />
        <span className="text-[10.5px] leading-snug text-muted">{strengthHelp(engineShown)}</span>
      </div>
      <Row
        label={
          <span className="flex items-center">
            Voice isolation
            {!pro && <ProBadge />}
          </span>
        }
      >
        <Switch
          size="sm"
          label="Voice isolation"
          checked={a.voiceIsolation.enabled && pro}
          disabled={!pro}
          onChange={(v) => setAudio({ voiceIsolation: { ...a.voiceIsolation, enabled: v } })}
        />
      </Row>
      <div className="flex flex-col gap-1">
        <span className="text-[12px]">Isolation margin</span>
        <RangeSlider
          label="Voice isolation margin"
          value={a.voiceIsolation.marginDb}
          min={3}
          max={15}
          step={0.5}
          format={(v) => `${v.toFixed(1)} dB`}
          disabled={!a.voiceIsolation.enabled}
          onCommit={(v) => setAudio({ voiceIsolation: { ...a.voiceIsolation, marginDb: v } })}
        />
        <span className="text-[10.5px] leading-snug text-muted">
          Mutes background speech that is quieter than your own voice while you pause. Smaller
          margins isolate more; 6 dB is a good start. It cannot separate voices talking at the
          same time.
        </span>
        {routing?.chainTelemetry && isolationLine(routing.chainTelemetry, a.voiceIsolation.marginDb) && (
          <span className="text-[10.5px] text-accent">
            {isolationLine(routing.chainTelemetry, a.voiceIsolation.marginDb)}
          </span>
        )}
      </div>
      {st && (
        <span className="flex items-center gap-2 text-[11px] text-muted">
          <span className={`h-1.5 w-1.5 rounded-full ${toneDot[st.tone]}`} />
          Engine: {st.label}
          {st.detail && st.tone !== "active" ? ` — ${st.detail}` : ""}
        </span>
      )}
      {t && <span className="text-[11px] text-muted">{telemetryLine(t)}</span>}
    </div>
  );
}

function RoutingDetails({ routing }: { routing: RoutingStatus }) {
  const r = routingSummary(routing);
  const st = routing.stats;
  const fmt = (f: { sampleRate: number; channels: number }) =>
    `${f.sampleRate / 1000} kHz · ${f.channels} ch`;
  return (
    <div className="flex flex-col gap-0.5 text-[11px] leading-snug text-muted">
      <span className="flex items-center gap-2 text-[12px] text-ink">
        <span className={`h-2 w-2 rounded-full ${toneDot[r.tone]}`} />
        {r.title}
      </span>
      <span className={toneText[r.tone]}>{r.detail}</span>
      {routing.info && (
        <span>
          Mic {fmt(routing.info.input)} → 48 kHz → cable {fmt(routing.info.output)}
        </span>
      )}
      {st && (
        <>
          <span>
            Shuusshy buffer {st.bufferedMs.toFixed(1)} ms · clock drift correction{" "}
            {st.driftPpm.toFixed(0)} ppm
          </span>
          <span className={hasGlitches(st) ? "text-warn" : ""}>
            Dropouts {st.underruns} · overflows {st.overruns} · resyncs {st.resyncs} · device glitches{" "}
            {st.xruns}
          </span>
          <span className="text-muted/70">
            VB-CABLE adds its own delay (about 80 ms measured on the development PC).
          </span>
        </>
      )}
    </div>
  );
}

/** Sign-in start: the switch reflects Windows' own Run entry, not a stored setting. */
function StartWithWindows({ portable }: { portable: boolean }) {
  const [on, setOn] = useState<boolean | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    getStartWithWindows().then(setOn).catch(() => setOn(false));
  }, []);
  return (
    <>
      <Row label="Start with Windows">
        <Switch
          size="sm"
          label="Start with Windows"
          checked={on ?? false}
          disabled={on === null}
          onChange={(v) =>
            setStartWithWindows(v)
              .then((r) => {
                setOn(r);
                setErr(null);
              })
              .catch((e) => setErr(String(e)))
          }
        />
      </Row>
      <span className={`text-[10.5px] leading-snug ${err ? "text-danger" : "text-muted"}`}>
        {err ??
          (portable
            ? "Starts this portable copy, from its current folder, quietly in the tray at sign-in."
            : "Starts quietly in the tray when you sign in, so the virtual mic is always ready.")}
      </span>
    </>
  );
}

function CableSetupSteps() {
  return (
    <ol className="list-decimal space-y-0.5 pl-4 text-[11px] leading-snug text-muted">
      <li>
        Download VB-CABLE from the official site:{" "}
        <span className="select-text text-ink">{VB_CABLE_URL}</span>{" "}
        <button
          type="button"
          onClick={() => openVbCableSite().catch(() => {})}
          className="press glass-btn-accent ml-1 rounded-full px-2 py-0.5 text-[10.5px] text-ink"
        >
          Open in browser
        </button>
      </li>
      <li>Extract the ZIP and run VBCABLE_Setup_x64.exe as administrator.</li>
      <li>Restart Windows if the installer asks you to.</li>
      <li>Shuusshy detects the cable automatically. No restart of Shuusshy is needed.</li>
      <li>If it stays missing, check that “CABLE Input” and “CABLE Output” are enabled in Windows Sound settings.</li>
    </ol>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="glass flex flex-col gap-2.5 px-3.5 py-3">
      <h2 className="label-tech flex items-center gap-2 text-ink/75">
        <span aria-hidden="true" className="h-1.5 w-1.5 rounded-full bg-accent shadow-[0_0_8px] shadow-accent" />
        {title}
      </h2>
      {children}
    </section>
  );
}

function Row({ label, children }: { label: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span>{label}</span>
      {children}
    </div>
  );
}

function Notice({
  tone,
  children,
  onDismiss,
}: {
  tone: "warning" | "error" | "info";
  children: React.ReactNode;
  onDismiss?: () => void;
}) {
  const color =
    tone === "error"
      ? "border-danger/40 bg-danger/10 text-danger"
      : tone === "info"
        ? "border-accent/40 bg-accent/10 text-ink"
        : "border-warn/40 bg-warn/10 text-warn";
  return (
    <div
      role={tone === "error" ? "alert" : "status"}
      className={`reveal mx-3 mt-2 flex items-start gap-2 rounded-lg border px-2.5 py-1.5 text-[11px] leading-snug ${color}`}
    >
      <span className="min-w-0 flex-1 select-text break-words">{children}</span>
      {onDismiss && (
        <button
          type="button"
          aria-label="Dismiss"
          title="Dismiss"
          onClick={onDismiss}
          className="-mr-1 shrink-0 rounded px-1 opacity-80 hover:bg-base/40 hover:opacity-100"
        >
          ×
        </button>
      )}
    </div>
  );
}

function IconButton({
  label,
  onClick,
  children,
  disabled,
  danger,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
  disabled?: boolean;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      disabled={disabled}
      className={`press grid h-7 w-7 place-items-center rounded-md text-muted
        hover:bg-white/10 hover:text-ink disabled:opacity-40
        ${danger ? "hover:bg-danger/80 hover:text-ink" : ""}`}
    >
      {children}
    </button>
  );
}
