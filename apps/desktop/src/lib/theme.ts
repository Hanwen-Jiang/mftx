import {isMac} from "./platform";

export type Theme = "light" | "dark";

const STORAGE_KEY = "mftx-theme";

/**
 * On macOS, force the native window appearance to match the in-app theme so the
 * vibrancy ("sidebar") material renders light/dark independent of the system —
 * a dark frosted sidebar under a light system, or vice-versa (the Codex look).
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

/** Resolve the startup theme: saved preference → OS preference → light. */
export function getInitialTheme(): Theme {
  if (typeof window !== "undefined") {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored === "light" || stored === "dark") return stored;
    if (window.matchMedia?.("(prefers-color-scheme: dark)").matches) return "dark";
  }
  return "light";
}

/** Apply a theme to <html> using HeroUI's class + data-theme conventions, and persist it. */
export function applyTheme(theme: Theme) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.classList.toggle("dark", theme === "dark");
  root.classList.toggle("light", theme === "light");
  root.dataset.theme = theme;
  root.style.colorScheme = theme;
  applyWindowTheme(theme);
  try {
    window.localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    /* localStorage unavailable — ignore */
  }
}
