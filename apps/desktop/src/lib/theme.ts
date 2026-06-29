import {isMac} from "./platform";

export type Theme = "light" | "dark";
export type ThemeMode = "auto" | "light" | "dark";

const MODE_KEY = "mftx-theme-mode";
const LEGACY_KEY = "mftx-theme";

/**
 * On macOS, force the native window appearance to match the resolved theme so
 * the vibrancy ("sidebar") material renders light/dark independent of the system.
 * No-op outside Tauri (e.g. the browser preview).
 */
function applyWindowTheme(theme: Theme) {
  if (!isMac || typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) return;
  void import("@tauri-apps/api/window")
    .then(({getCurrentWindow}) => {
      try {
        return getCurrentWindow().setTheme(theme);
      } catch {
        return undefined;
      }
    })
    .catch(() => undefined);
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

function applyResolvedTheme(theme: Theme) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.classList.toggle("dark", theme === "dark");
  root.classList.toggle("light", theme === "light");
  root.dataset.theme = theme;
  root.style.colorScheme = theme;
  applyWindowTheme(theme);
}

/** Apply a theme MODE: resolve it, paint `<html>`, and persist the mode. */
export function applyThemeMode(mode: ThemeMode) {
  applyResolvedTheme(resolveTheme(mode));
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
    if (getMode() === "auto") applyResolvedTheme(systemTheme());
  });
}
