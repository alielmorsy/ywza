import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { approve } from "../api";
import { Composer } from "./Composer";
import { Feed } from "./Feed";
import { Scope, useDriveHint } from "./Scope";
import { isActive, pendingOf, useRuns, type Run } from "./runs";
import { openRun, setFocus, useLabel, useM } from "./store";

const PEEK = 148;
const DOCKED = 76;

/** Chat sheet with three detents (peek, half, full). On a run's page it shrinks to a docked composer. */
export function Sheet() {
  const L = useLabel();
  const tab = useM((m) => m.tab);
  const runId = useM((m) => m.run);
  const live = useM((m) => m.live);
  const focus = useM((m) => m.focus);
  const pairing = useM((m) => m.pairing);
  const sel = useM((m) => m.sel);
  const runs = useRuns();
  const hint = useDriveHint();
  const [vh, setVh] = useState(innerHeight);
  const [h, setH] = useState(PEEK);
  const [dragging, setDragging] = useState(false);
  const g = useRef<{ y0: number; h0: number; ly: number; lt: number; v: number; moved: boolean }>(undefined);
  const dets = [PEEK, Math.round(vh * 0.5), vh - 150];
  const mode = h < 260 ? "peek" : h < vh * 0.7 ? "half" : "full";

  const detail = tab === "runs" ? runs.find((r) => r.id === runId) : undefined;
  const docked = !!detail && !live;
  const visible = !pairing && (!!live || docked || (tab === "wall" && !runId));
  const liveRun = live ? runs.find((r) => isActive(r) && (r.current === live.dev || r.driver === live.dev)) : undefined;
  const focused = runs.find((r) => r.id === focus);
  const shown: Run | undefined = (live ? liveRun : detail) ?? focused ?? runs[0];
  const pend = runs.flatMap((r) => pendingOf(r).map((a) => ({ a, r })))[0];

  useEffect(() => { const f = () => setVh(innerHeight); addEventListener("resize", f); return () => removeEventListener("resize", f); }, []);
  useEffect(() => {
    document.documentElement.style.setProperty("--sheet-h", visible ? `${docked ? DOCKED : h}px` : "0px");
    return () => { document.documentElement.style.setProperty("--sheet-h", "0px"); };
  }, [visible, docked, h]);

  const snap = (to: number) => { setDragging(false); setH(to); };
  const cycle = () => snap(dets[(dets.indexOf(h) + 1) % 3] ?? PEEK);
  const down = (e: PointerEvent) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    g.current = { y0: e.clientY, h0: h, ly: e.clientY, lt: performance.now(), v: 0, moved: false };
  };
  const move = (e: PointerEvent) => {
    const d = g.current;
    if (!d) return;
    const dy = d.y0 - e.clientY, now = performance.now();
    if (Math.abs(dy) > 4) { d.moved = true; setDragging(true); }
    d.v = (d.ly - e.clientY) / Math.max(1, now - d.lt);
    d.ly = e.clientY;
    d.lt = now;
    if (d.moved) setH(Math.max(PEEK - 20, Math.min(dets[2] + 20, d.h0 + dy)));
  };
  const up = () => {
    const d = g.current;
    g.current = undefined;
    if (!d) return;
    if (!d.moved) return cycle();
    const p = h + d.v * 200;
    snap(dets.reduce((a, b) => (Math.abs(b - p) < Math.abs(a - p) ? b : a)));
  };
  const key = (e: KeyboardEvent) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); cycle(); } };

  if (!visible) return null;
  if (docked) {
    return (
      <div className="absolute inset-x-0 bottom-[var(--tabs-h)] z-30 border-t border-line bg-chrome">
        <Composer run={detail} scope={[]} />
      </div>
    );
  }

  return (
    <div className="absolute inset-x-0 bottom-[var(--tabs-h)] z-30 flex flex-col rounded-t-2xl border-t border-line bg-chrome shadow-[0_-8px_24px_rgba(0,0,0,.35)]"
      style={{ height: h, transition: dragging ? "none" : "height .3s var(--ease)" }} data-mode={mode}>
      <button className="grid h-6 w-full flex-none place-items-center" style={{ touchAction: "none" }} aria-label="Resize chat sheet"
        onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={up} onKeyDown={key}>
        <i className="h-1 w-9 rounded-full bg-line" />
      </button>
      <Scope />
      {hint && <p className="flex-none px-3.5 pb-1.5 text-xs text-warn" role="status">{hint}</p>}
      {mode === "peek" ? (
        <div className={`flex min-h-8 flex-none items-center justify-between gap-2 px-3.5 pb-1 text-xs ${pend ? "text-warn" : "text-text-2"}`}>
          {pend ? (
            <>
              <span className="min-w-0 truncate">{L(pend.r.current)} needs approval: {pend.a.tool}</span>
              <button className="m-btn primary sm" onClick={() => approve(pend.a.request, true, false, pend.a.device)}>Allow</button>
            </>
          ) : shown ? (
            <button className="min-w-0 truncate text-left" onClick={() => openRun(shown.id)}>
              <b className="font-semibold text-text">{shown.title}</b>{shown.last && ` · ${L(shown.current)}: ${shown.last}`}
            </button>
          ) : <span>{sel.length ? "Ready" : "No runs yet"}</span>}
        </div>
      ) : <Feed run={shown} />}
      <Composer run={live ? liveRun : focused} scope={live ? [live.dev] : sel} onNew={live ? undefined : () => setFocus()}
        onSent={() => { if (h < dets[1] && !live) snap(dets[1]); }} />
    </div>
  );
}
