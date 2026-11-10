// UI state: selected device, per-device view, selected session, layout toggles, status line.
import { useSyncExternalStore } from "react";
import { LOCAL, onRemoved, type ApprovalMode, type Effort } from "./api";

export type View = "chat" | "live" | "access";
export interface DeviceUi {
  view: View;
  /** Session shown in chat, the live view's docked chat and the split view. null = new chat. */
  session: string | null;
  /** Chat view: right half shows the live view of the window the AI works in. */
  split: boolean;
  /** Split view: draw element boxes over the window. */
  debug: boolean;
  /** Live view: chat docked on the right. */
  dock: boolean;
}
export interface Nav {
  /** Selected device in the rail, or "settings". */
  selected: string;
  ui: Record<string, DeviceUi>;
  /** Composer choices, shared by every chat. undefined = agent default. */
  model?: string;
  effort?: Effort;
  approval?: ApprovalMode;
  status: { text: string; tone: "" | "warn" | "bad"; pulse: boolean };
}

const READY: Nav["status"] = { text: "Ready", tone: "", pulse: false };
const DEFAULT_UI: DeviceUi = { view: "chat", session: null, split: false, debug: false, dock: false };
const saved = (() => { try { return JSON.parse(localStorage.getItem("ywza.composer") ?? "{}"); } catch { return {}; } })();

let nav: Nav = { selected: LOCAL, ui: {}, model: saved.model, effort: saved.effort, approval: saved.approval, status: READY };
const subs = new Set<() => void>();
const set = (n: Partial<Nav>) => { nav = { ...nav, ...n }; subs.forEach((f) => f()); };

const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };
export function useNav<T>(sel: (n: Nav) => T): T {
  return useSyncExternalStore(subscribe, () => sel(nav));
}
onRemoved.add((name) => {
  const { [name]: _, ...ui } = nav.ui;
  set({ ui, selected: nav.selected === name ? LOCAL : nav.selected });
});
export const uiOf = (n: Nav, dev: string): DeviceUi => n.ui[dev] ?? DEFAULT_UI;
export const select = (selected: string) => set({ selected });
export const setUi = (dev: string, p: Partial<DeviceUi>) => set({ ui: { ...nav.ui, [dev]: { ...uiOf(nav, dev), ...p } } });
/** Opens `dev` in `view`, optionally on `session`. */
export const openDevice = (dev: string, view?: View, session?: string | null) =>
  set({ selected: dev, ui: { ...nav.ui, [dev]: { ...uiOf(nav, dev), ...(view && { view }), ...(session !== undefined && { session }) } } });
export function setComposer(p: { model?: string; effort?: Effort; approval?: ApprovalMode }) {
  set(p);
  try { localStorage.setItem("ywza.composer", JSON.stringify({ model: nav.model, effort: nav.effort, approval: nav.approval })); } catch { /* private mode */ }
}
export const setStatus = (text: string, tone: Nav["status"]["tone"] = "", pulse = false) => set({ status: { text, tone, pulse } });
export const readyStatus = () => set({ status: READY });
/** Shows `text` in the status bar for a moment. */
let flashTimer: ReturnType<typeof setTimeout> | undefined;
export const flash = (text: string, tone: Nav["status"]["tone"] = "") => {
  clearTimeout(flashTimer);
  setStatus(text, tone);
  flashTimer = setTimeout(readyStatus, tone === "bad" ? 6000 : 1800);
};
