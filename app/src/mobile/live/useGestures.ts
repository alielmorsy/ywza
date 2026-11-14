import { useRef, useState, type PointerEvent, type RefObject } from "react";
import { FULL, pointOn } from "../../views/live/input";
import { click, emit } from "./remote";
import { clamp, edgePan, NONE, pinch, zoomAt, type Pt, type View } from "./zoom";

const HOLD_MS = 450;
const SLOP = 10; // px a finger may wander and still tap
const PX_PER_NOTCH = 30;

type Mode = "idle" | "tap" | "hold" | "drag" | "hover" | "scroll" | "pan" | "swipe" | "pinch";
interface G {
  pts: Map<number, Pt>;
  mode: Mode;
  id: number;
  start: Pt & { cx: number; cy: number };
  last: { cx: number; cy: number };
  pin?: { v0: View; m0: Pt; d0: number };
  timer?: number;
  edge?: number;
  wheel: { dx: number; dy: number; queued: boolean };
  move?: { x: number; y: number };
  tap?: { t: number; cx: number; cy: number; at: { x: number; y: number } };
}

const mid = (a: Pt, b: Pt) => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
const dist = (a: Pt, b: Pt) => Math.hypot(a.x - b.x, a.y - b.y);

/**
 * Touch on the live screen. Two fingers always pinch-zoom and pan the picture here.
 * With `control`, one finger drives the device: tap clicks, drag scrolls (moves the pointer
 * when zoomed), hold then drag selects (left-button drag), hold and let go right-clicks.
 * While zoomed, a dragging or pointing finger near an edge pans the picture that way. Without it one finger pans a
 * zoomed picture, double-tap zooms, and a horizontal swipe calls `onSwipe`.
 */
export function useGestures(o: {
  dev: string; box: RefObject<HTMLDivElement | null>; canvas: RefObject<HTMLCanvasElement | null>;
  control: boolean; onSwipe: (dir: 1 | -1) => void; onTap: () => void;
}) {
  const [view, setViewState] = useState<View>(NONE);
  const vr = useRef(view);
  const setView = (v: View) => { vr.current = v; setViewState(v); };
  const [ping, setPing] = useState<{ x: number; y: number; k: number } | null>(null);
  const g = useRef<G>({ pts: new Map(), mode: "idle", id: -1, start: { x: 0, y: 0, cx: 0, cy: 0 }, last: { cx: 0, cy: 0 }, wheel: { dx: 0, dy: 0, queued: false } }).current;

  const size = () => ({ w: o.box.current?.clientWidth ?? 1, h: o.box.current?.clientHeight ?? 1 });
  const local = (e: PointerEvent): Pt => { const b = o.box.current!.getBoundingClientRect(); return { x: e.clientX - b.left, y: e.clientY - b.top }; };
  const remote = (cx: number, cy: number) => { const c = o.canvas.current; const p = c && pointOn(c, { clientX: cx, clientY: cy }, FULL); return p?.inside ? p : null; };
  const stopTimer = () => { clearTimeout(g.timer); g.timer = undefined; };
  const flushWheel = () => requestAnimationFrame(() => {
    const { dx, dy } = g.wheel;
    g.wheel = { dx: 0, dy: 0, queued: false };
    if (dx || dy) emit(o.dev, { type: "wheel", dx, dy });
  });
  const flushMove = () => requestAnimationFrame(() => { if (g.move) emit(o.dev, { type: "mouse_move", ...g.move }); g.move = undefined; });
  const pointAt = (cx: number, cy: number) => { const at = remote(cx, cy); if (at) { if (!g.move) flushMove(); g.move = { x: at.x, y: at.y }; } };
  // Keeps panning while the finger rests at an edge, and moves the pointer onto what slides under it.
  const edgeLoop = () => {
    g.edge = undefined;
    if (g.mode !== "drag" && g.mode !== "hover") return;
    const b = o.box.current!.getBoundingClientRect(), v = vr.current;
    const n = edgePan(v, { x: g.last.cx - b.left, y: g.last.cy - b.top }, size());
    if (n.tx === v.tx && n.ty === v.ty) return;
    setView(n);
    pointAt(g.last.cx, g.last.cy);
    g.edge = requestAnimationFrame(edgeLoop);
  };
  const showPing = (p: Pt) => setPing({ ...p, k: Date.now() });

  const down = (e: PointerEvent<HTMLDivElement>) => {
    if ((e.target as Element).closest("button, a, input, textarea")) return; // controls drawn over the screen
    e.stopPropagation();
    e.preventDefault(); // keeps focus (and the soft keyboard) on the typing field
    e.currentTarget.setPointerCapture(e.pointerId);
    const p = local(e);
    g.pts.set(e.pointerId, p);
    if (g.pts.size === 2) {
      stopTimer();
      if (g.mode === "drag") emit(o.dev, { type: "mouse_button", button: "left", down: false });
      const [a, b] = [...g.pts.values()];
      g.mode = "pinch";
      g.pin = { v0: vr.current, m0: mid(a, b), d0: dist(a, b) };
      return;
    }
    if (g.pts.size > 2) return;
    g.mode = "tap";
    g.id = e.pointerId;
    g.start = { ...p, cx: e.clientX, cy: e.clientY };
    g.last = { cx: e.clientX, cy: e.clientY };
    if (o.control) g.timer = window.setTimeout(() => {
      if (g.mode !== "tap") return;
      g.mode = "hold";
      navigator.vibrate?.(15);
      showPing(g.start);
    }, HOLD_MS);
  };

  const move = (e: PointerEvent<HTMLDivElement>) => {
    if (!g.pts.has(e.pointerId)) return;
    g.pts.set(e.pointerId, local(e));
    if (g.mode === "pinch") {
      if (g.pts.size !== 2 || !g.pin) return;
      const [a, b] = [...g.pts.values()];
      setView(pinch(g.pin.v0, g.pin.m0, g.pin.d0, mid(a, b), dist(a, b), size()));
      return;
    }
    if (e.pointerId !== g.id) return;
    const dx = e.clientX - g.last.cx, dy = e.clientY - g.last.cy;
    const far = Math.hypot(e.clientX - g.start.cx, e.clientY - g.start.cy) > SLOP;
    if (g.mode === "tap" && far) { stopTimer(); g.mode = o.control ? vr.current.s > 1 ? "hover" : "scroll" : vr.current.s > 1 ? "pan" : "swipe"; }
    if (g.mode === "hold" && far) {
      const at = remote(g.start.cx, g.start.cy);
      if (!at) { g.mode = "idle"; return; }
      g.mode = "drag";
      emit(o.dev, { type: "mouse_move", x: at.x, y: at.y });
      emit(o.dev, { type: "mouse_button", button: "left", down: true });
    }
    if (g.mode === "scroll") { // natural scrolling: content follows the finger
      g.wheel.dx -= dx / PX_PER_NOTCH; g.wheel.dy -= dy / PX_PER_NOTCH;
      if (!g.wheel.queued) { g.wheel.queued = true; flushWheel(); }
    } else if (g.mode === "pan") {
      const v = vr.current;
      setView(clamp({ s: v.s, tx: v.tx + dx, ty: v.ty + dy }, size()));
    } else if (g.mode === "drag" || g.mode === "hover") pointAt(e.clientX, e.clientY);
    g.last = { cx: e.clientX, cy: e.clientY };
    if ((g.mode === "drag" || g.mode === "hover") && g.edge === undefined) g.edge = requestAnimationFrame(edgeLoop);
  };

  const up = (e: PointerEvent<HTMLDivElement>, cancel = false) => {
    if (!g.pts.delete(e.pointerId)) return;
    if (g.mode === "pinch") { if (!g.pts.size) g.mode = "idle"; return; }
    if (e.pointerId !== g.id) return;
    stopTimer();
    const mode = g.mode;
    g.mode = "idle";
    if (mode === "drag") {
      const at = remote(e.clientX, e.clientY);
      if (at && !cancel) emit(o.dev, { type: "mouse_move", x: at.x, y: at.y });
      emit(o.dev, { type: "mouse_button", button: "left", down: false });
      return;
    }
    if (cancel) return;
    const now = Date.now(), t = g.tap;
    const again = !!t && now - t.t < 400 && Math.hypot(e.clientX - t.cx, e.clientY - t.cy) < 30;
    if (mode === "tap" && o.control) {
      // A finger lands a few device pixels off each time: a quick second tap reuses the first point so it double-clicks.
      const at = again ? t!.at : remote(e.clientX, e.clientY);
      if (!at) return;
      click(o.dev, at);
      showPing(g.start);
      g.tap = { t: now, cx: e.clientX, cy: e.clientY, at: { x: at.x, y: at.y } };
      o.onTap();
    } else if (mode === "tap") {
      if (again) { setView(vr.current.s > 1 ? NONE : zoomAt(vr.current, g.start, 2.5, size())); g.tap = undefined; }
      else g.tap = { t: now, cx: e.clientX, cy: e.clientY, at: { x: 0, y: 0 } };
    } else if (mode === "hold") {
      const at = remote(e.clientX, e.clientY);
      if (at) click(o.dev, at, "right");
    } else if (mode === "swipe") {
      const dx = e.clientX - g.start.cx, dy = e.clientY - g.start.cy;
      if (Math.abs(dx) > 50 && Math.abs(dy) < 40) o.onSwipe(dx < 0 ? 1 : -1);
    }
  };

  /** Re-fits the view to a resized box, without a render when nothing changes. */
  const refit = () => {
    const v = vr.current, c = clamp(v, size());
    if (c.s !== v.s || c.tx !== v.tx || c.ty !== v.ty) setView(c);
  };

  return {
    view, setView, refit, current: () => vr.current, ping,
    handlers: {
      onPointerDown: down,
      onPointerMove: move,
      onPointerUp: (e: PointerEvent<HTMLDivElement>) => up(e),
      onPointerCancel: (e: PointerEvent<HTMLDivElement>) => up(e, true),
      onContextMenu: (e: { preventDefault: () => void }) => e.preventDefault(),
    },
  };
}
