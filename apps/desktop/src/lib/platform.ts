const ua = typeof navigator !== "undefined" ? navigator.userAgent : "";

/** macOS uses native window decorations (traffic lights), so we hide our custom chrome there. */
export const isMac = /Macintosh|Mac OS X/i.test(ua);
export const isWindows = /Windows/i.test(ua);
