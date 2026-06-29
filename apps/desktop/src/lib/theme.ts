import {isMac} from "./platform";

export type Theme = "light" | "dark";
export type ThemeMode = "auto" | "light" | "dark";

const MODE_KEY = "mftx-theme-mode";
const LEGACY_KEY = "mftx-theme";

/**
 * macOS: drive the native window appearance so the vibrancy ("sidebar") material
 * renders the right light/dark.
 *  - explicit "light"/"dark" → force that theme.
 *  - "auto" → setTheme(null) so the window FOLLOWS the OS. Forcing a concrete
 *    theme here locks the webview's `prefers-color-scheme` to that value — which
 *    is exactly why "auto" stopped following the system before.
 * No-op outside Tauri (e.g. the browser preview).
 */
function applyWindowTheme(mode: ThemeMode, resolved: Theme): Promise<void> {
  if (!isMac || typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
    return Promise.resolve();
  }
  const target: Theme | null = mode === "auto" ? null : resolved;
  return import("@tauri-apps/api/window")
    .then(({getCurrentWindow}) => getCurrentWindow().setTheme(target))
    .catch(() => undefined)
    .then(() => undefined);
}

/** The OS-preferred theme right now. */
export function systemTheme(): Theme {
  if (typeof window !== "undefined" && window.matchMedia?.("(prefers-color-scheme: dark)").matches) {
    return "dark";
  }
  return "light";
}

/** Resolve a mode to a concrete light/dark theme ("auto" follows the OS). */
export function resolveTheme(mode: ThemeMode): Theme {
  return mode === "auto" ? systemTheme() : mode;
}

/** Startup mode: saved mode → migrated legacy binary preference → "auto". */
export function getInitialThemeMode(): ThemeMode {
  if (typeof window === "undefined") return "auto";
  const stored = window.localStorage.getItem(MODE_KEY);
  if (stored === "auto" || stored === "light" || stored === "dark") return stored;
  const legacy = window.localStorage.getItem(LEGACY_KEY);
  if (legacy === "light" || legacy === "dark") return legacy;
  return "auto";
}

/** Paint the resolved theme onto `<html>` (classes + data-theme + color-scheme). */
function paintTheme(theme: Theme) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.classList.toggle("dark", theme === "dark");
  root.classList.toggle("light", theme === "light");
  root.dataset.theme = theme;
  root.style.colorScheme = theme;
}

/** Apply a theme MODE: paint `<html>`, sync the native window theme, persist. */
export function applyThemeMode(mode: ThemeMode) {
  paintTheme(resolveTheme(mode));
  void applyWindowTheme(mode, resolveTheme(mode)).then(() => {
    // Releasing the forced theme ("auto") lets the webview's matchMedia reflect
    // the real OS again — repaint in case the previously-forced value differed.
    if (mode === "auto") paintTheme(systemTheme());
  });
  try {
    window.localStorage.setItem(MODE_KEY, mode);
  } catch {
    /* localStorage unavailable — ignore */
  }
}

let systemListenerBound = false;

/** Re-apply the resolved theme when the OS appearance changes, while in "auto". */
export function bindSystemThemeListener(getMode: () => ThemeMode) {
  if (systemListenerBound || typeof window === "undefined" || !window.matchMedia) return;
  systemListenerBound = true;
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (getMode() !== "auto") return;
    const theme = systemTheme();
    paintTheme(theme);
    void applyWindowTheme("auto", theme);
  });
}
