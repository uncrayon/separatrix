// Keyboard shortcuts for the document: a pure matcher (no React), so node's
// unit tests import it; useShortcuts.ts installs it.

export type ShortcutAction = "new" | "open" | "save" | "saveAs" | "export";

export type ShortcutEvent = Pick<
  KeyboardEvent,
  "key" | "code" | "ctrlKey" | "metaKey" | "shiftKey" | "altKey" | "isComposing" | "keyCode"
>;

export interface ShortcutEnv {
  /** Cmd is the modifier (macOS); otherwise Ctrl. */
  mac: boolean;
  /** In the desktop app. The browser keeps Ctrl+N for a new window, so New has no shortcut there. */
  tauri: boolean;
}

/**
 * The letter a key press stands for: `e.key` lowercased when it is an ASCII
 * letter, otherwise the physical key (`e.code`, e.g. `KeyS`), so the
 * shortcuts also work on Cyrillic, Greek and similar layouts.
 */
export function shortcutLetter(e: Pick<KeyboardEvent, "key" | "code">): string | null {
  const k = (e.key ?? "").toLowerCase();
  if (/^[a-z]$/.test(k)) return k;
  const m = /^Key([A-Z])$/.exec(e.code ?? "");
  return m ? m[1].toLowerCase() : null;
}

/** Mod+N, Mod+O, Mod+S, Mod+Shift+S and Mod+E; null for anything else, or while an IME is composing. */
export function matchShortcut(e: ShortcutEvent, env: ShortcutEnv): ShortcutAction | null {
  if (e.isComposing || e.keyCode === 229) return null;
  const mod = env.mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
  if (!mod || e.altKey) return null;
  switch (shortcutLetter(e)) {
    case "s":
      return e.shiftKey ? "saveAs" : "save";
    case "o":
      return e.shiftKey ? null : "open";
    case "n":
      return e.shiftKey || !env.tauri ? null : "new";
    case "e":
      return e.shiftKey ? null : "export";
    default:
      return null;
  }
}

/** The platform uses Cmd for shortcuts. */
/** The shortcut as the platform writes it: `⌘⇧S` on macOS, `Ctrl+Shift+S` elsewhere. */
export function shortcutLabel(letter: string, opts: { mac: boolean; shift?: boolean }): string {
  const l = letter.toUpperCase();
  if (opts.mac) return `⌘${opts.shift ? "⇧" : ""}${l}`;
  return `Ctrl+${opts.shift ? "Shift+" : ""}${l}`;
}

export function isMacPlatform(nav: { platform?: string; userAgent?: string } | undefined): boolean {
  const p = nav?.platform || nav?.userAgent || "";
  return /Mac|iPhone|iPad|iPod/i.test(p);
}
