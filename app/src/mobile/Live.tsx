import { useCallback, useEffect, useLayoutEffect, useRef, useState, type PointerEvent } from "react";
import { useStore } from "../api";
import { Icon } from "../Icon";
import { useAccess } from "../views/live/access";
import { drawFull } from "../views/live/draw";
import { MonitorPicker } from "../views/live/MonitorPicker";
import type { Sink } from "../views/live/streams";
import { setMonitor, useLiveStream } from "../views/live/useLiveStream";
import { ApproveCard } from "./ApproveCard";
import { useDeviceNames } from "./devices";
import { devicesOf, pendingOf, useRuns } from "./runs";
import { closeLive, lockLive, openRun, switchLive, useLabel, useM, type LiveState } from "./store";
import { tileInfo, DEV_COLOR, DEV_TEXT } from "./tiles";
import { StepLine } from "./ui";

const EASE = "cubic-bezier(.2,.8,.2,1)";
const calm = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
type Orientation = { lock?: (o: string) => Promise<void>; unlock?: () => void };
const orient = () => screen.orientation as unknown as Orientation | undefined;
/** Android: MainActivity's bridge hides the system bars, which the WebView's own fullscreen cannot. */
const bars = () => (window as unknown as { ywzaWindow?: { immersive: (on: boolean) => void } }).ywzaWindow;
const tileRect = (dev: string) => document.querySelector(`[data-tile="${CSS.escape(dev)}"] button`)?.getBoundingClientRect();

/** Moves `el` between its place and the tile rect, so the screen grows out of the tile and shrinks back into it. */
function fly(el: HTMLElement, from: DOMRect, enter: boolean, done?: () => void) {
  const to = el.getBoundingClientRect();
  const away = `translate(${from.left - to.left}px, ${from.top - to.top}px) scale(${from.width / to.width})`;
  const a = el.animate(enter ? [{ transform: away }, { transform: "none" }] : [{ transform: "none" }, { transform: away, opacity: 0.6 }], { duration: 340, easing: EASE, fill: enter ? "none" : "forwards" });
  if (done) a.onfinish = done;
}

export function Live() {
  const live = useM((m) => m.live);
  return live ? <Screen live={live} /> : null;
}

function Screen({ live }: { live: LiveState }) {
  const { dev } = live;
  const L = useLabel();
  const names = useDeviceNames();
  const runs = useRuns();
  const { d, connected, canView } = useAccess(dev);
  const shot = useStore((s) => s.shots[dev]);
  const box = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const slide = useRef({ dev, at: names.indexOf(dev), cls: "" });
  const swipe = useRef<{ x: number; y: number }>(undefined);
  const [full, setFull] = useState(false);

  const sink = useCallback<Sink>((f) => { if (canvas.current) drawFull(canvas.current, f); }, []);
  const ok = connected && canView;
  const { live: streaming, monitor } = useLiveStream(dev, sink, ok);
  const monitors = d?.monitors;
  const mon = monitors?.find((m) => m.index === monitor);
  const info = tileInfo(dev, d, runs);
  const run = runs.find((r) => r.id === live.follow) ?? info.run ?? runs.find((r) => devicesOf(r).includes(dev));
  const followed = runs.find((r) => r.id === live.follow);
  const steps = (info.run ?? run)?.steps.filter((s) => s.device === dev).slice(-6) ?? [];
  const pend = runs.flatMap(pendingOf).filter((a) => runs.find((r) => r.id === a.session)?.current === dev);

  useLayoutEffect(() => { if (live.from && box.current && !calm()) fly(box.current, live.from, true); }, []); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { if (followed && !live.locked && followed.current !== dev && names.includes(followed.current)) switchLive(followed.current); }, [followed?.current]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => { // the stream may remember a monitor this device does not have
    if (ok && monitors?.length && !mon) setMonitor(dev, (monitors.find((m) => m.primary) ?? monitors[0]).index);
  }, [ok, monitors, mon, dev]);
  useEffect(() => { if (!ok) setFull(false); }, [ok]);
  useEffect(() => {
    if (!full) return;
    // The overlay alone fills the app; real fullscreen also hides the system bars where the webview allows it.
    bars()?.immersive(true);
    document.documentElement.requestFullscreen?.().then(() => orient()?.lock?.(mon && mon.height > mon.width ? "portrait" : "landscape")).catch(() => {});
    const exit = () => { if (!document.fullscreenElement) setFull(false); };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") setFull(false); };
    document.addEventListener("fullscreenchange", exit);
    addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("fullscreenchange", exit);
      removeEventListener("keydown", esc);
      orient()?.unlock?.();
      bars()?.immersive(false);
      if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
    };
  }, [full]);

  const back = () => {
    const to = tileRect(dev);
    if (box.current && to && !calm()) fly(box.current, to, false, closeLive); else closeLive();
  };
  const step = (n: number) => {
    if (full) { // fullscreen swipes go through the monitors
      const at = monitors?.findIndex((m) => m.index === monitor) ?? -1;
      const next = monitors?.[at + n];
      if (at >= 0 && next) setMonitor(dev, next.index);
      return;
    }
    const i = names.indexOf(dev) + n;
    if (names[i]) switchLive(names[i]);
  };
  const at = names.indexOf(dev);
  if (slide.current.dev !== dev) slide.current = { dev, at, cls: at > slide.current.at ? "m-slide-l" : "m-slide-r" };
  const dir = slide.current.cls;
  const down = (e: PointerEvent) => { swipe.current = { x: e.clientX, y: e.clientY }; };
  const up = (e: PointerEvent) => {
    const s = swipe.current;
    swipe.current = undefined;
    if (s && Math.abs(e.clientX - s.x) > 50 && Math.abs(e.clientY - s.y) < 40) step(e.clientX < s.x ? 1 : -1);
  };

  return (
    <div className={`absolute inset-x-0 top-[env(safe-area-inset-top)] bottom-[var(--tabs-h)] flex flex-col bg-ground ${full ? "z-[55]" : "z-20"}`} onPointerDown={down} onPointerUp={up}>
      <header className="flex h-12 flex-none items-center gap-2 px-2.5">
        <button type="button" className="m-btn sm !rounded-full !pl-2.5" onClick={back}><Icon name="back" />Back</button>
        <div className="min-w-0 flex-1 text-center">
          <b className="truncate text-sm font-semibold">{L(dev)}</b>{" "}
          <span className="text-[11px]" style={{ color: DEV_COLOR[info.state] }}>{DEV_TEXT[info.state]}</span>
        </div>
        {live.follow
          ? <button type="button" className={`m-btn sm ${live.locked ? "" : "primary"}`} aria-pressed={live.locked} onClick={() => lockLive(!live.locked)}>{live.locked ? "Locked" : "Following AI"}</button>
          : <span className="flex" role="group" aria-label="Devices">
              {names.map((n) => (
                <button key={n} type="button" className="grid h-11 w-5 place-items-center" aria-label={`Show ${L(n)}`} aria-current={n === dev} onClick={() => switchLive(n)}>
                  <i className={`size-2 rounded-full ${n === dev ? "bg-primary" : "bg-line"}`} />
                </button>
              ))}
            </span>}
      </header>

      {ok && (
        <div className="m-mon flex flex-none items-center gap-2 px-2.5 pb-1.5">
          <div className="m-scroll min-w-0 flex-1"><MonitorPicker dev={dev} value={monitor} onPick={(i) => setMonitor(dev, i)} /></div>
          <button type="button" className="m-btn sm" onClick={() => setFull(true)}><Icon name="expand" />Full screen</button>
        </div>
      )}

      <div key={dev} className={`flex min-h-0 flex-1 flex-col ${dir}`}>
        {/* One canvas for both modes: a static screen sends no new frame to fill a fresh one. */}
        <div ref={box} className={full ? "fixed inset-0 z-10 overflow-hidden bg-black" : "relative flex-none overflow-hidden bg-backdrop"}
          style={{ touchAction: full ? "none" : "pan-y", transformOrigin: "0 0" }}>
          {!ok ? (
            <div className="grid aspect-[16/10] place-items-center px-6 text-center text-text-2">{!connected ? `${L(dev)} is offline. No live feed.` : `${L(dev)} does not let you view its screen.`}</div>
          ) : (
            <>
              {!streaming && (shot ? <img src={shot.src} alt="" className={full ? "size-full object-contain" : "block w-full"} /> : <div className="aspect-[16/10]" />)}
              <canvas ref={canvas} className={`${full ? "size-full object-contain" : "block h-auto w-full"} ${streaming ? "" : "hidden"}`} aria-label={`Screen of ${L(dev)}`} />
              {!streaming && <span className="absolute inset-x-0 bottom-2 text-center text-xs text-text-2" role="status">{d?.liveError ?? "Connecting"}</span>}
              {full && (
                <div className="m-mon absolute inset-x-0 top-0 flex items-center gap-2 bg-linear-to-b from-black/70 to-transparent px-[max(10px,env(safe-area-inset-left))] pt-[max(6px,env(safe-area-inset-top))] pb-4">
                  <button type="button" className="m-btn sm !rounded-full" onClick={() => setFull(false)} aria-label="Exit full screen"><Icon name="shrink" /></button>
                  <b className="min-w-0 truncate text-sm font-semibold">{L(dev)}</b>
                  <div className="m-scroll ml-auto min-w-0"><MonitorPicker dev={dev} value={monitor} onPick={(i) => setMonitor(dev, i)} /></div>
                </div>
              )}
              {full && pend[0] && <div className="absolute inset-x-3 bottom-[max(12px,env(safe-area-inset-bottom))] mx-auto max-w-md"><ApproveCard a={pend[0]} device={dev} /></div>}
              {info.here && info.run && <span className="mono absolute bottom-2 left-2 max-w-[calc(100%-16px)] truncate rounded-[3px] border border-line bg-chrome/90 px-1.5 py-0.5 text-[10.5px] text-primary">{info.run.last}</span>}
            </>
          )}
        </div>

        <div className="m-scroll m-rise flex min-h-0 flex-1 flex-col gap-2 px-3.5 pt-3 pb-[calc(var(--sheet-h)+16px)]">
          {pend.map((a) => <ApproveCard key={a.request} a={a} device={dev} />)}
          <div className="rounded-xl border border-line bg-surface px-3 py-2.5">
            <small className="block text-[10.5px] tracking-[.05em] text-text-2 uppercase">Now</small>
            <div className="mono text-xs">{info.ticker}</div>
            {run && (
              <div className="mt-1.5 text-xs">
                {info.run ? "Run" : "Last run"}: <button type="button" className="inline-flex min-h-6 items-center text-primary underline" onClick={() => openRun(run.id)}>{run.title}</button>
              </div>
            )}
          </div>
          {steps.length > 0 && <div className="flex flex-col">{steps.map((s) => <StepLine key={s.key} step={s} tag={false} />)}</div>}
          {names.length > 1 && <p className="mt-1 text-center text-[11px] text-text-2">Swipe left or right to change device</p>}
          {(monitors?.length ?? 0) > 1 && <p className="text-center text-[11px] text-text-2">In full screen, swipe to change monitor{mon ? ` (now ${mon.index + 1} of ${monitors!.length})` : ""}</p>}
        </div>
      </div>
    </div>
  );
}
