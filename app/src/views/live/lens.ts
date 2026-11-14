import { useEffect, useMemo, useState } from "react";
import { getState, lensAct, send, useStore, type LensElement, type Monitor, type Rect } from "../../api";

/** A monitor rect (pixels) in the unzoomed box of the canvas, where object-contain may letterbox the picture. */
export function toBox(c: HTMLCanvasElement, mon: Monitor, r: Rect) {
  const W = c.clientWidth, H = c.clientHeight;
  if (!c.width || !c.height || !W || !H) return null;
  const k = Math.min(W / c.width, H / c.height), w = c.width * k, h = c.height * k;
  return { x: (W - w) / 2 + (r.x / mon.width) * w, y: (H - h) / 2 + (r.y / mon.height) * h, w: (r.width / mon.width) * w, h: (r.height / mon.height) * h };
}

export const chipName = (el: LensElement) => el.name || el.role;
const haystack = (el: LensElement) => `${el.name} ${el.role}`.toLowerCase();

/**
 * Smart lens of a device's live view: while `on` and the stream runs, the agent follows the focused window and
 * sends its tappable elements. `fail` gets the text of a refused action.
 */
export function useLens(dev: string, o: { live: boolean; monitor: number; canAct: boolean; fail: (text: string) => void }) {
  const [on, setOn] = useState(false);
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<number | null>(null);
  const lens = useStore((s) => s.devices[dev]?.lens);
  const active = on && o.live;

  useEffect(() => {
    if (!active) return;
    send(dev, { type: "lens_start" });
    return () => { if (getState().devices[dev]?.connected) send(dev, { type: "lens_stop" }); };
  }, [dev, active, o.monitor]);
  // Agents without the lens drop lens_start without a reply; say so instead of waiting forever.
  const [quiet, setQuiet] = useState(false);
  useEffect(() => {
    setQuiet(false);
    if (!active || lens) return;
    const t = setTimeout(() => { setQuiet(true); o.fail(`No lens from ${dev}. Update its ywza agent if this lasts.`); }, 8000);
    return () => clearTimeout(t);
  }, [dev, active, !!lens, o.monitor]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { if (!on) { setQuery(""); setEditing(null); } }, [on]);
  useEffect(() => setEditing(null), [dev]);

  const all = useMemo(
    () => (lens && lens.monitor === o.monitor ? Object.values(lens.elements).filter((e) => e.rect.width > 0 && e.rect.height > 0) : []),
    [lens, o.monitor],
  );
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const shown = useMemo(
    () => (words.length ? all.filter((e) => words.every((w) => haystack(e).includes(w))) : all),
    [all, query], // eslint-disable-line react-hooks/exhaustive-deps
  );
  const editEl = all.find((e) => e.id === editing);

  async function run(el: LensElement, text?: string, submit?: boolean) {
    if (!lens || !o.canAct) return false;
    const r = await lensAct(dev, lens.window, el.id, { text, submit });
    if (!r.ok) o.fail(r.text || "The element did not respond");
    return r.ok;
  }
  const tap = (el: LensElement) => { if (!o.canAct) return; if (el.editable) setEditing(el.id); else run(el); };
  const type = async (text: string, submit: boolean) => {
    if (editEl && await run(editEl, text, submit)) setEditing(null);
  };

  return { on, setOn, active, lens, quiet, all, shown, query, setQuery, filtering: words.length > 0, editEl, setEditing, tap, type, canAct: o.canAct };
}
export type LensCtl = ReturnType<typeof useLens>;
