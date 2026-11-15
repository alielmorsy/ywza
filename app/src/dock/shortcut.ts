import type { DockKey } from "../api";

export const SHORTCUTS: [DockKey, string][] = [
  ["alt+backquote", "Alt + ` (the key under Esc)"],
  ["ctrl+alt+space", "Ctrl + Alt + Space"],
  ["alt+space", "Alt + Space"],
];
export const DEFAULT_SHORTCUT: DockKey = "alt+backquote";
const STORE = "ywza.dock.shortcut";

export function savedShortcut(): DockKey {
  try {
    const k = localStorage.getItem(STORE);
    return SHORTCUTS.find(([x]) => x === k)?.[0] ?? DEFAULT_SHORTCUT;
  } catch { return DEFAULT_SHORTCUT; }
}
export const saveShortcut = (k: DockKey) => { try { localStorage.setItem(STORE, k); } catch { /* private mode */ } };
