import type { Settings } from "../types";

let systemCleanup: (() => void) | null = null;

/**
 * Resolve a stored theme ("system" | "light" | "dark") and apply it to the
 * current document: the `dark` class drives the CSS tokens (see index.css)
 * and `color-scheme` keeps native form controls in sync. For "system" a
 * `prefers-color-scheme` listener keeps the UI live when Windows switches.
 *
 * Each Tauri window is a separate document, so this must run per window
 * (main + quick picker).
 */
export function applyTheme(theme: Settings["theme"]): void {
  systemCleanup?.();
  systemCleanup = null;

  const root = document.documentElement;
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const apply = (dark: boolean) => {
    root.classList.toggle("dark", dark);
    root.style.colorScheme = dark ? "dark" : "light";
  };

  if (theme === "dark") {
    apply(true);
  } else if (theme === "light") {
    apply(false);
  } else {
    apply(mq.matches);
    const onChange = (e: MediaQueryListEvent) => apply(e.matches);
    mq.addEventListener("change", onChange);
    systemCleanup = () => mq.removeEventListener("change", onChange);
  }
}
