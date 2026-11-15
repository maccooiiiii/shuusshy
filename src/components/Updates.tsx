import { useEffect, useRef, useState } from "react";
import {
  type ReleaseInfo,
  type Settings,
  type UpdateInfo,
  checkForUpdates,
  downloadUpdate,
  exportSettings,
  importSettings,
  installUpdate,
  onUpdateProgress,
  openReleasesPage,
} from "../lib/ipc";
import { errorText } from "../lib/status";
import { Switch } from "./Switch";

/** "Valid until …" / "Unofficial build" line for the installed release. */
export function releaseLine(r: ReleaseInfo | null): string {
  if (!r) return "";
  switch (r.validity) {
    case "unofficial":
      return "Unofficial build: no release date, so no validity period applies.";
    case "expired":
      return `This version expired on ${r.expires}.`;
    default:
      return `Official ${r.channel ?? ""} release of ${r.releaseDate}, valid until ${r.expires} (${r.daysLeft} days left).`;
  }
}

/**
 * Expiry notices (spec Part 10.2/10.3): a discreet reminder 30 days before, a prominent
 * warning 7 days before, and the expired card. Nothing is shown for valid or unofficial builds.
 */
export function ExpiryNotice({
  release,
  onExport,
}: {
  release: ReleaseInfo | null;
  onExport: () => void;
}) {
  if (!release) return null;
  const open = () => openReleasesPage().catch(() => {});
  if (release.validity === "reminder") {
    return (
      <button
        type="button"
        onClick={open}
        className="press reveal px-1 text-left text-[11px] text-muted hover:text-ink"
      >
        This version is valid until {release.expires}. Update when convenient →
      </button>
    );
  }
  if (release.validity === "warning") {
    return (
      <div className="glass reveal flex flex-col gap-1.5 px-3 py-2.5" role="status">
        <span className="label-tech text-warn">Update soon</span>
        <span className="text-[11px] leading-snug text-ink/90">
          This version of Shuusshy expires in {release.daysLeft} day
          {release.daysLeft === 1 ? "" : "s"} ({release.expires}). Afterwards, noise cancellation
          and the virtual microphone stop until you update.
        </span>
        <button type="button" onClick={open} className="press glass-btn-accent self-start rounded-full px-3 py-1 text-[11.5px]">
          Get the latest version
        </button>
      </div>
    );
  }
  if (release.validity === "expired") {
    return (
      <div className="glass reveal flex flex-col gap-2 px-3 py-3" role="alert">
        <span className="label-tech text-danger">This version has expired</span>
        <span className="text-[11px] leading-snug text-ink/90">
          Shuusshy {release.version} was valid until {release.expires}. Noise cancellation and
          the virtual microphone are stopped. Your settings are kept, and installing the latest
          version restores everything.
        </span>
        <div className="flex flex-wrap gap-2">
          <button type="button" onClick={open} className="press glass-btn-accent rounded-full px-3 py-1 text-[11.5px]">
            Get the latest version
          </button>
          <button type="button" onClick={onExport} className="press glass-btn rounded-full px-3 py-1 text-[11.5px]">
            Export settings
          </button>
        </div>
      </div>
    );
  }
  return null;
}

/** Settings > Application and Updates: check, download (with progress) and install. */
export function UpdatePanel({
  release,
  settings,
  onChange,
  announced,
}: {
  release: ReleaseInfo | null;
  settings: Settings | null;
  onChange: (s: Settings) => void;
  announced: UpdateInfo | null;
}) {
  const [state, setState] = useState<
    | { kind: "idle" }
    | { kind: "checking" }
    | { kind: "upToDate" }
    | { kind: "available"; info: UpdateInfo }
    | { kind: "downloading"; info: UpdateInfo; pct: number | null }
    | { kind: "ready"; info: UpdateInfo }
    | { kind: "error"; message: string }
  >({ kind: "idle" });
  const configured = !!release?.updatesEnabled;

  useEffect(() => {
    if (announced) setState({ kind: announced.ready ? "ready" : "available", info: announced });
  }, [announced]);

  useEffect(() => {
    const un = onUpdateProgress((p) =>
      setState((s) =>
        s.kind === "downloading"
          ? { ...s, pct: p.total ? Math.min(100, Math.round((p.downloaded / p.total) * 100)) : null }
          : s,
      ),
    );
    return () => {
      un.then((f) => f());
    };
  }, []);

  const check = () => {
    setState({ kind: "checking" });
    checkForUpdates()
      .then((i) => setState(i ? { kind: "available", info: i } : { kind: "upToDate" }))
      .catch((e) => setState({ kind: "error", message: errorText(e) }));
  };
  const download = (info: UpdateInfo) => {
    setState({ kind: "downloading", info, pct: 0 });
    downloadUpdate()
      .then((i) => setState({ kind: "ready", info: i }))
      .catch((e) => setState({ kind: "error", message: errorText(e) }));
  };
  const install = () =>
    installUpdate().catch((e) => setState({ kind: "error", message: errorText(e) }));
  const setApp = (patch: Partial<Settings["app"]>) =>
    settings && onChange({ ...settings, app: { ...settings.app, ...patch } });

  return (
    <div className="flex flex-col gap-2">
      <span className={`text-[10.5px] leading-snug ${release?.validity === "expired" ? "text-danger" : "text-muted"}`}>
        {releaseLine(release)}
      </span>
      {!configured ? (
        <span className="text-[10.5px] leading-snug text-muted">
          Update checks are not set up in this build (no repository address), so Shuusshy makes
          no network connections.
        </span>
      ) : (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={check}
              disabled={state.kind === "checking" || state.kind === "downloading"}
              className="press glass-btn rounded-full px-3 py-1 text-[11.5px] disabled:opacity-40"
            >
              {state.kind === "checking" ? "Checking…" : "Check for updates"}
            </button>
            {release?.releasesUrl && (
              <button
                type="button"
                onClick={() => openReleasesPage().catch(() => {})}
                className="press text-[11px] text-muted hover:text-ink"
              >
                Release notes on GitHub →
              </button>
            )}
          </div>
          {state.kind === "upToDate" && <span className="text-[11px] text-accent">Shuusshy is up to date.</span>}
          {state.kind === "error" && <span className="text-[11px] text-danger">{state.message}</span>}
          {(state.kind === "available" || state.kind === "downloading" || state.kind === "ready") && (
            <div className="well reveal flex flex-col gap-1.5 rounded-xl px-2.5 py-2">
              <span className="text-[12px]">
                Shuusshy <span className="text-accent">{state.info.version}</span> is available
                <span className="text-muted"> (you have {state.info.currentVersion})</span>
              </span>
              {state.info.notes && (
                <details className="text-[10.5px] text-muted">
                  <summary className="cursor-pointer text-ink/80">What's new</summary>
                  <p className="mt-1 max-h-28 overflow-y-auto whitespace-pre-wrap select-text">{state.info.notes}</p>
                </details>
              )}
              {state.kind === "available" && state.info.portable && (
                <>
                  <span className="text-[10.5px] text-muted">
                    Portable edition: download the new ZIP and replace shuusshy.exe, keeping the
                    data folder.
                  </span>
                  <button type="button" onClick={() => openReleasesPage().catch(() => {})} className="press glass-btn-accent self-start rounded-full px-3 py-1 text-[11.5px]">
                    Open the Releases page
                  </button>
                </>
              )}
              {state.kind === "available" && !state.info.portable && (
                <button type="button" onClick={() => download(state.info)} className="press glass-btn-accent self-start rounded-full px-3 py-1 text-[11.5px]">
                  Download update
                </button>
              )}
              {state.kind === "downloading" && (
                <div className="well h-1.5 overflow-hidden rounded-full" role="progressbar" aria-valuenow={state.pct ?? undefined}>
                  <div className="h-full rounded-full bg-accent transition-[width]" style={{ width: `${state.pct ?? 30}%` }} />
                </div>
              )}
              {state.kind === "ready" && (
                <>
                  <span className="text-[10.5px] text-muted">
                    Downloaded and signature-checked. Installing closes Shuusshy for a moment and
                    keeps your settings.
                  </span>
                  <button type="button" onClick={install} className="press glass-btn-accent self-start rounded-full px-3 py-1 text-[11.5px]">
                    Restart and update
                  </button>
                </>
              )}
            </div>
          )}
          <Row label="Check for updates automatically">
            <Switch
              size="sm"
              label="Check for updates automatically"
              checked={settings?.app.checkUpdates ?? false}
              disabled={!settings}
              onChange={(v) => setApp({ checkUpdates: v })}
            />
          </Row>
          <Row label="Download updates automatically">
            <Switch
              size="sm"
              label="Download updates automatically"
              checked={settings?.app.autoDownloadUpdates ?? false}
              disabled={!settings || !settings.app.checkUpdates}
              onChange={(v) => setApp({ autoDownloadUpdates: v })}
            />
          </Row>
          <span className="text-[10.5px] leading-snug text-muted">
            Checks contact GitHub at most once a day for release information only. Installing an
            update always needs your click.
          </span>
        </>
      )}
    </div>
  );
}

/** Export to Documents\Shuusshy, or import from a previously exported file. */
export function BackupControls({ onImported }: { onImported: (s: Settings) => void }) {
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const file = useRef<HTMLInputElement>(null);
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={() =>
            exportSettings()
              .then((p) => setMsg({ ok: true, text: `Saved to ${p}` }))
              .catch((e) => setMsg({ ok: false, text: errorText(e) }))
          }
          className="press glass-btn rounded-full px-3 py-1 text-[11.5px]"
        >
          Export settings
        </button>
        <button type="button" onClick={() => file.current?.click()} className="press glass-btn rounded-full px-3 py-1 text-[11.5px]">
          Import settings…
        </button>
        <input
          ref={file}
          type="file"
          accept=".json,application/json"
          className="hidden"
          onChange={(e) => {
            const f = e.target.files?.[0];
            e.target.value = "";
            if (!f) return;
            f.text()
              .then(importSettings)
              .then((s) => {
                onImported(s);
                setMsg({ ok: true, text: "Settings imported." });
              })
              .catch((err) => setMsg({ ok: false, text: errorText(err) }));
          }}
        />
      </div>
      {msg && (
        <span className={`select-text break-all text-[10.5px] ${msg.ok ? "text-accent" : "text-danger"}`}>{msg.text}</span>
      )}
      <span className="text-[10.5px] leading-snug text-muted">
        The exported file has no Pro key and no device IDs. Importing keeps your microphone and key.
      </span>
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3 text-[13px]">
      <span>{label}</span>
      {children}
    </div>
  );
}
