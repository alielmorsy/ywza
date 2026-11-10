import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { send, type Element, type Monitor, type WindowInfo } from "../../api";
import { useAccess } from "./access";
import { drawCrop, drawFull } from "./draw";
import { FULL, pointOn, type Region } from "./input";
import { unavailable } from "./Notice";
import type { Sink } from "./streams";
import { useLiveStream } from "./useLiveStream";
import { useRemoteInput } from "./useRemoteInput";

const POLL_MS = 1000, STALE_MS = 4500;

const Note = ({ children }: { children: string }) => (
  <p className="absolute bottom-3 left-1/2 -translate-x-1/2 rounded-md border border-line bg-chrome/90 px-2.5 py-1 text-xs whitespace-nowrap text-text-2" role="status">{children}</p>
);

const label = (el: Element) => (el.name ? `${el.name} (${el.role})` : el.role);

/** Live stream cropped to a window's rect, optionally with its UI element boxes. */
export function WindowLive({ dev, window: win, debug, active }: { dev: string; window: string | null; debug: boolean; active: boolean }) {
  const { d, name, connected, canView } = useAccess(dev);
  const ok = connected && canView;
  const run = ok && active;
  const info: WindowInfo | undefined = win ? d?.windows[win] : undefined;
  const mon: Monitor | undefined = d?.monitors?.find((m) => m.index === info?.monitor);
  const rect = info?.rect;

  const canvas = useRef<HTMLCanvasElement>(null);
  const geo = useRef({ win, rect, mon });
  geo.current = { win, rect, mon };
  const sink = useCallback<Sink>((f) => {
    const c = canvas.current, { win, rect, mon } = geo.current;
    if (!c) return;
    if (!win) drawFull(c, f);
    else if (rect) drawCrop(c, f, rect, mon ? f.displayWidth / mon.width : 1);
  }, []);
  const region = useCallback((): Region => {
    const { rect, mon } = geo.current;
    return rect && mon ? { x: rect.x / mon.width, y: rect.y / mon.height, w: rect.width / mon.width, h: rect.height / mon.height } : FULL;
  }, []);

  useLiveStream(dev, sink, run, win ? info?.monitor : 0);

  const [closed, setClosed] = useState(false);
  const seen = useRef(Date.now());
  useEffect(() => { seen.current = Date.now(); setClosed(false); }, [win, run]);
  useEffect(() => { if (info) seen.current = Date.now(); }, [info]);
  useEffect(() => {
    if (!run || !win) return;
    const poll = () => {
      send(dev, { type: "window_info", window: win, elements: debug });
      setClosed(Date.now() - seen.current > STALE_MS);
    };
    poll();
    const t = setInterval(poll, POLL_MS);
    return () => clearInterval(t);
  }, [dev, win, debug, run]);
  useEffect(() => { if (ok && !d?.monitors) send(dev, { type: "list_monitors" }); }, [dev, ok, d?.monitors]);

  const cropped = !!win && !!rect && !closed;
  const mapped = !win || (!!rect && !!mon);
  const input = useRemoteInput(dev, canvas, region, false); // split view only watches; take over from Live

  const boxes = useMemo(
    () => (debug ? [...(info?.elements ?? [])].sort((a, b) => b.rect.width * b.rect.height - a.rect.width * a.rect.height) : []),
    [debug, info?.elements],
  );
  const [hot, setHot] = useState<number | null>(null);
  const hit = (e: { clientX: number; clientY: number }) => {
    const c = canvas.current, r = region();
    const p = c && mon && pointOn(c, e, r);
    if (!p || !p.inside) return null;
    const x = p.x * mon.width, y = p.y * mon.height;
    let best: Element | null = null;
    for (const el of boxes) {
      const q = el.rect;
      if (x >= q.x && x < q.x + q.width && y >= q.y && y < q.y + q.height && (!best || q.width * q.height <= best.rect.width * best.rect.height)) best = el;
    }
    return best?.id ?? null;
  };
  const hotEl = boxes.find((el) => el.id === hot);

  const off = unavailable(name, connected, canView, d?.detail);
  if (off) return off;

  const ar = rect ? rect.width / rect.height : 0;
  return (
    <div className="relative h-full min-h-0 overflow-hidden bg-backdrop" style={{ containerType: "size" }}>
      <div className="absolute inset-0 grid place-items-center">
        <div className={`relative ${cropped ? "" : "size-full"} ${closed ? "invisible" : ""}`}
          style={cropped ? { width: `min(100cqw, calc(100cqh * ${ar}))`, aspectRatio: String(ar) } : undefined}>
          <canvas ref={canvas} aria-label={win ? `Window ${info?.title ?? ""}` : `Screen of ${name}`} title={hotEl ? label(hotEl) : undefined}
            tabIndex={-1} className="size-full object-contain outline-none focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent"
            {...input}
            onPointerMove={(e) => { input.onPointerMove?.(e); if (debug) setHot(hit(e)); }}
            onPointerLeave={() => setHot(null)} />
          {cropped && rect && boxes.length > 0 && (
            <div className="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden>
              {boxes.map((el) => {
                const on = el.id === hot;
                return (
                  <div key={el.id} className={`absolute border ${on ? "z-10 border-accent-bg bg-accent-soft" : "border-accent"}`}
                    style={{
                      left: `${((el.rect.x - rect.x) / rect.width) * 100}%`, top: `${((el.rect.y - rect.y) / rect.height) * 100}%`,
                      width: `${(el.rect.width / rect.width) * 100}%`, height: `${(el.rect.height / rect.height) * 100}%`,
                    }}>
                    <span className={`mono absolute top-0 left-0 px-0.5 text-[9px] leading-[12px] whitespace-nowrap text-ink ${on ? "bg-accent-bg" : "bg-accent"}`}>
                      {on && el.name ? `${el.id} ${el.name}` : el.id}
                    </span>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
      {closed && <div className="absolute inset-0 grid place-items-center text-text-2" role="status">{d?.outdated ? "This device runs an older ywza agent and cannot follow windows. Update it." : "Window closed"}</div>}
      {!closed && !win && <Note>Follows the agent's window once it uses one</Note>}
      {!closed && win && !info && <Note>Finding the window</Note>}
    </div>
  );
}
