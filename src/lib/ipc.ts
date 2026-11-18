// Typed wrappers around the Rust commands in src-tauri/src/commands.rs.
// Types mirror src-tauri/src/settings/model.rs (serde camelCase).
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Engine = "rnnoise" | "deepFilterNet";

export interface AppSettings {
  alwaysOnTop: boolean;
  /** Compact banner: status, microphone, on/off and test only. */
  compact: boolean;
  /** Closing the banner hides it to the tray (processing continues) instead of exiting. */
  keepRunningInTray: boolean;
  /** Stored Pro key (set only through activateLicense). */
  licenseKey: string | null;
  activationPromptDismissed: boolean;
  /** See-through glass window (desktop blurred behind the banner). */
  glass: boolean;
  /** Strength of the dark tint over the blurred desktop, 0.35–0.95. */
  glassTint: number;
  accent: Accent;
  /** Look for a new release on GitHub at start-up (at most daily). */
  checkUpdates: boolean;
  /** Download a found update in the background (installing needs a click). */
  autoDownloadUpdates: boolean;
  /** Unix seconds of the last update check (set by the backend). */
  lastUpdateCheck: number;
}

export type Accent = "cyan" | "violet" | "mint" | "amber" | "rose" | "ice" | "system";

/** The accent in use as #rrggbb ("system" resolved to the Windows accent). */
export const getAccentColor = () => invoke<string>("get_accent_color");

export interface NoiseGate {
  enabled: boolean;
  thresholdDb: number;
}

export interface AudioSettings {
  inputDeviceId: string | null;
  virtualDeviceId: string | null;
  inputDeviceName: string | null;
  routingEnabled: boolean;
  engine: Engine;
  suppressionEnabled: boolean;
  suppressionStrength: number;
  inputGainDb: number;
  outputGainDb: number;
  noiseGate: NoiseGate;
  voiceIsolation: VoiceIsolation;
}

export interface VoiceIsolation {
  enabled: boolean;
  /** How far below your tracked voice level other speech is muted (dB). */
  marginDb: number;
}

export interface Settings {
  app: AppSettings;
  audio: AudioSettings;
}

export interface AppInfo {
  version: string;
  dataDir: string;
  settingsPersistent: boolean;
  /** Running from the portable ZIP (settings stored next to the executable). */
  portable: boolean;
  platform?: string;
  /** This copy ships VB-Audio's official VB-CABLE setup (Windows installer and portable ZIP). */
  bundledCable?: boolean;
}

/** True when running inside the Shuusshy desktop shell (not a plain browser tab). */
export const inDesktopShell = (): boolean => isTauri();

export const getAppInfo = () => invoke<AppInfo>("get_app_info");
export const getSettings = () => invoke<Settings>("get_settings");
export const updateSettings = (settings: Settings) =>
  invoke<Settings>("update_settings", { settings });
export const resetSettings = () => invoke<Settings>("reset_settings");
export const resizeBanner = (height: number) => invoke<void>("resize_banner", { height });

export interface PresetInfo {
  id: string;
  name: string;
  description: string;
  engine: Engine;
  strength: number;
  gateEnabled: boolean;
  gateThresholdDb: number;
  isolationEnabled: boolean;
  isolationMarginDb: number;
}

export interface PresetState {
  presets: PresetInfo[];
  /** Id of the preset matching the current settings, or "custom". */
  current: string;
}

export const getPresets = () => invoke<PresetState>("get_presets");
export const applyPreset = (id: string) => invoke<Settings>("apply_preset", { id });

/** Name of the active preset, or null for custom settings. */
export const presetName = (p: PresetState | null): string | null =>
  p?.presets.find((x) => x.id === p.current)?.name ?? null;

/** Settings changed outside the banner (system tray). */
export const onSettingsChanged = (cb: (s: Settings) => void) =>
  listen<Settings>("settings-changed", (e) => cb(e.payload));

/** The tray asked to show the settings panel. */
export const onOpenSettings = (cb: () => void) => listen("open-settings", () => cb());

export interface LicenseInfo {
  /** Master keys never expire; Pro keys have an expiry date. Same features. */
  kind: "master" | "pro";
  serial: number;
  issued: string;
  expires: string | null;
}

export type LicenseStatus =
  | { state: "free" }
  | ({ state: "pro" } & LicenseInfo)
  | ({ state: "expired" } & LicenseInfo);

export const getLicense = () => invoke<LicenseStatus>("get_license");
export const activateLicense = (key: string) => invoke<LicenseStatus>("activate_license", { key });
export const removeLicense = () => invoke<LicenseStatus>("remove_license");

/** Presets that use Pro features (DeepFilterNet or voice isolation). */
export const presetNeedsPro = (p: PresetInfo) =>
  p.engine === "deepFilterNet" || p.isolationEnabled;

/** Whether Windows starts this copy of Shuusshy at sign-in (read from Windows). */
export const getStartWithWindows = () => invoke<boolean>("get_start_with_windows");
export const setStartWithWindows = (enabled: boolean) =>
  invoke<boolean>("set_start_with_windows", { enabled });
/** Opens the official VB-CABLE page in the browser. */
export const openVbCableSite = () => invoke<void>("open_vb_cable_site");
/** Opens VB-Audio's own VB-CABLE setup (included with Shuusshy) with the administrator prompt. */
export const installBundledCable = () => invoke<void>("install_bundled_cable");

export interface ReleaseInfo {
  version: string;
  channel: string | null;
  releaseDate: string | null;
  expires: string | null;
  daysLeft: number | null;
  validity: "unofficial" | "valid" | "reminder" | "warning" | "expired";
  repo: string | null;
  releasesUrl: string | null;
  /** Update checks are possible in this build. */
  updatesEnabled: boolean;
}

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  date: string | null;
  notes: string | null;
  ready: boolean;
  /** Portable edition: update by downloading the new ZIP, not via the installer. */
  portable: boolean;
}

export const getReleaseInfo = () => invoke<ReleaseInfo>("get_release_info");
export const openReleasesPage = () => invoke<void>("open_releases_page");
export const exportSettings = () => invoke<string>("export_settings");
export const importSettings = (json: string) => invoke<Settings>("import_settings", { json });
export const checkForUpdates = () => invoke<UpdateInfo | null>("check_for_updates");
export const downloadUpdate = () => invoke<UpdateInfo>("download_update");
export const installUpdate = () => invoke<void>("install_update");
export const onUpdateAvailable = (cb: (u: UpdateInfo) => void) =>
  listen<UpdateInfo>("update-available", (e) => cb(e.payload));
export const onUpdateProgress = (cb: (p: { downloaded: number; total: number | null }) => void) =>
  listen<{ downloaded: number; total: number | null }>("update-progress", (e) => cb(e.payload));
