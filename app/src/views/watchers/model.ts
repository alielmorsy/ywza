import { useCallback, useEffect, useState } from "react";
import { callTool, useStore } from "../../api";

type Condition =
  | { type: "window_appears" | "window_closes"; title: string }
  | { type: "element_appears" | "element_changes"; title: string; text: string }
  | { type: "text"; title: string; text: string; ocr?: boolean }
  | { type: "term_output"; term: number; text: string }
  | { type: "term_exit"; term: number; code: number | null }
  | { type: "schedule"; every_min: number | null; at: string | null };
type Action = { type: "notify"; text?: string | null } | { type: "prompt"; text: string; device?: string | null } | { type: "plan"; name: string };

export interface Watcher {
  id: string; name: string; enabled: boolean; when: Condition; then: Action;
  min_gap_s: number; max_per_hour: number; fired: number; last_fired_ms: number | null;
}

const q = (s: string) => `"${s}"`;
const cut = (s: string, n = 48) => (s.length > n ? `${s.slice(0, n)}...` : s);

export function describeWhen(c: Condition): string {
  switch (c.type) {
    case "window_appears": return `Window ${q(c.title)} opens`;
    case "window_closes": return `Window ${q(c.title)} closes`;
    case "element_appears": return `${q(c.text)} appears in ${q(c.title)}`;
    case "element_changes": return `${q(c.text)} changes in ${q(c.title)}`;
    case "text": return `Text ${q(c.text)} in ${q(c.title)}${c.ocr ? " (OCR)" : ""}`;
    case "term_output": return `Terminal t${c.term} prints ${q(c.text)}`;
    case "term_exit": return `Terminal t${c.term} exits${c.code == null ? " with an error" : ` with ${c.code}`}`;
    case "schedule": return c.every_min ? `Every ${c.every_min} min` : `Daily at ${c.at}`;
  }
}

export function describeThen(a: Action): string {
  if (a.type === "notify") return a.text ? `Notify: ${cut(a.text)}` : "Notify";
  if (a.type === "plan") return `Run plan ${a.name}`;
  return `Ask the AI${a.device ? ` on ${a.device}` : ""}: ${cut(a.text)}`;
}

/** The watchers of `dev`, loaded when `open` and after every change or firing. */
export function useWatchers(dev: string, open: boolean) {
  const [list, setList] = useState<Watcher[] | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const lastFired = useStore((s) => s.devices[dev]?.fired?.[0]?.at);

  const load = useCallback(async () => {
    const r = await callTool(dev, "watcher_list", { json: true });
    if (!r.ok) return setError(r.text);
    try { setList(JSON.parse(r.text)); setError(""); } catch { setError("The device sent a list this app cannot read."); }
  }, [dev]);
  useEffect(() => { if (open) load(); }, [open, load, lastFired]);

  const change = async (name: string, args: unknown) => {
    setBusy(true);
    const r = await callTool(dev, name, args);
    if (!r.ok) setError(r.text);
    await load();
    setBusy(false);
  };
  return {
    list, error, busy, reload: load,
    toggle: (w: Watcher) => change("watcher_set", { id: w.id, enabled: !w.enabled }),
    remove: (w: Watcher) => change("watcher_delete", { id: w.id }),
  };
}
