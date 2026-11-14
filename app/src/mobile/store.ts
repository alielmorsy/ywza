// Phone UI state: tab, open run, live view, wall selection, chosen driver. Server data stays in api.ts.
import { useSyncExternalStore } from "react";
import { onRemoved, type Sid } from "../api";

export type Tab = "wall" | "runs" | "inbox";
/** `follow`: the live view moves with the device that run acts on, unless `locked`. `kb`: the remote keyboard is open (the chat sheet hides). */
export interface LiveState { dev: string; from?: DOMRect; follow?: Sid; locked: boolean; kb?: boolean }
interface M {
  tab: Tab;
  run: Sid | null;
  live: LiveState | null;
  pairing: boolean;
  menu: string | null;
  /** Wall selection: extra devices the driver may use. */
  sel: string[];
  driver?: string;
  /** The run the wall sheet shows and continues; null: a new run, unset: the latest run. */
  focus?: Sid | null;
  toast?: { text: string; at: number };
  /** Display names; the protocol has no rename, so these live on this phone only. */
  alias: Record<string, string>;
}

const load = <T,>(k: string, d: T): T => { try { return JSON.parse(localStorage.getItem(k) ?? "") ?? d; } catch { return d; } };
const save = (k: string, v: unknown) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch { /* private mode */ } };

let m: M = { tab: "wall", run: null, live: null, pairing: false, menu: null, sel: [], driver: load<string | undefined>("ywza.m.driver", undefined), alias: load("ywza.m.alias", {}) };
const subs = new Set<() => void>();
const set = (p: Partial<M>) => { m = { ...m, ...p }; subs.forEach((f) => f()); };
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };
export const useM = <T,>(sel: (m: M) => T): T => useSyncExternalStore(subscribe, () => sel(m));
export const getM = () => m;

onRemoved.add((name) => {
  const { [name]: _, ...alias } = m.alias;
  save("ywza.m.alias", alias);
  set({ alias, sel: m.sel.filter((n) => n !== name), menu: null, live: m.live?.dev === name ? null : m.live, driver: m.driver === name ? undefined : m.driver });
});

export const setTab = (tab: Tab) => set({ tab, run: null, live: null });
export const openRun = (run: Sid) => set({ tab: "runs", run, live: null, focus: run });
export const closeRun = () => set({ run: null });
export const openLive = (dev: string, o: { from?: DOMRect; follow?: Sid } = {}) => set({ live: { dev, locked: false, ...o } });
export const switchLive = (dev: string) => m.live && set({ live: { ...m.live, dev } });
export const lockLive = (locked: boolean) => m.live && set({ live: { ...m.live, locked } });
export const closeLive = () => set({ live: null });
export const setKb = (kb: boolean) => m.live && set({ live: { ...m.live, kb } });
export const setPairing = (pairing: boolean) => set({ pairing });
export const setMenu = (menu: string | null) => set({ menu });
export const setFocus = (focus?: Sid | null) => set({ focus });
export const toggleSel = (name: string) => set({ sel: m.sel.includes(name) ? m.sel.filter((n) => n !== name) : [...m.sel, name] });
export const clearSel = () => set({ sel: [] });
export const setDriver = (driver: string) => { save("ywza.m.driver", driver); set({ driver }); };
export const rename = (name: string, to: string) => {
  const { [name]: _, ...rest } = m.alias;
  const alias = to.trim() && to.trim() !== name ? { ...rest, [name]: to.trim() } : rest;
  save("ywza.m.alias", alias);
  set({ alias });
};
export const toast = (text: string) => { const at = Date.now(); set({ toast: { text, at } }); setTimeout(() => { if (m.toast?.at === at) set({ toast: undefined }); }, 2200); };
export const useLabel = () => { const alias = useM((x) => x.alias); return (n: string) => alias[n] ?? n; };
