import { useEffect, useRef } from "react";
import { isMacPlatform, matchShortcut, type ShortcutAction } from "./shortcuts";

export type ShortcutHandlers = Partial<Record<ShortcutAction, () => void>>;

/** A modal dialog is open (the gallery, the export dialog, a confirm): the document shortcuts wait. */
function modalOpen(): boolean {
  return document.querySelector('[aria-modal="true"]') !== null;
}

/**
 * Mod+N (desktop app only), Mod+O, Mod+S, Mod+Shift+S and Mod+E, on `window` in the
 * capture phase, so an equation field (MathLive) can't swallow them first.
 * Ignored during IME composition and while a modal dialog is open.
 */
export function useShortcuts(handlers: ShortcutHandlers, opts: { tauri: boolean; enabled?: boolean }) {
  const latest = useRef(handlers);
  latest.current = handlers;
  const enabled = opts.enabled ?? true;
  const tauri = opts.tauri;
  useEffect(() => {
    if (!enabled) return;
    const mac = isMacPlatform(typeof navigator === "undefined" ? undefined : navigator);
    const onKey = (e: KeyboardEvent) => {
      const action = matchShortcut(e, { mac, tauri });
      if (!action) return;
      const run = latest.current[action];
      if (!run || modalOpen()) return;
      e.preventDefault();
      e.stopPropagation();
      if (!e.repeat) run();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [enabled, tauri]);
}
