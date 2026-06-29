import {getCurrentWindow} from "@tauri-apps/api/window";
import {Copy, Minus, Square, X} from "lucide-react";
import {useEffect, useState} from "react";

/** The current Tauri window, or null when running outside Tauri (e.g. the dev/browser preview). */
function appWindow() {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    try {
      return getCurrentWindow();
    } catch {
      return null;
    }
  }
  return null;
}

/** Toggle maximize/restore — used by double-clicking the drag region. */
export function toggleMaximizeWindow() {
  appWindow()?.toggleMaximize();
}

/**
 * Custom min / maximize-restore / close controls for the frameless window
 * (`decorations: false`). Close hides the window to the tray, matching the
 * app's CloseRequested handler.
 */
export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const win = appWindow();
    if (!win) return;
    let unlisten: (() => void) | undefined;
    win.isMaximized().then(setMaximized).catch(() => undefined);
    win
      .onResized(() => {
        win.isMaximized().then(setMaximized).catch(() => undefined);
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => undefined);
    return () => unlisten?.();
  }, []);

  return (
    <div className="mftx-window-controls no-drag">
      <button
        aria-label="最小化"
        className="mftx-window-control"
        type="button"
        onClick={() => appWindow()?.minimize()}
      >
        <Minus className="size-4" />
      </button>
      <button
        aria-label={maximized ? "向下还原" : "最大化"}
        className="mftx-window-control"
        type="button"
        onClick={() => appWindow()?.toggleMaximize()}
      >
        {maximized ? <Copy className="size-3.5" /> : <Square className="size-3.5" />}
      </button>
      <button
        aria-label="关闭"
        className="mftx-window-control mftx-window-control--close"
        type="button"
        onClick={() => appWindow()?.close()}
      >
        <X className="size-4" />
      </button>
    </div>
  );
}
