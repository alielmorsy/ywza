// Pinch-zoom of the live screen: a scale and offset over a box of size w x h (CSS px).

export interface View { s: number; tx: number; ty: number }
export interface Size { w: number; h: number }
export interface Pt { x: number; y: number }

export const NONE: View = { s: 1, tx: 0, ty: 0 };
export const MAX = 6;

/** Keeps the picture covering the box: no zooming out past 1, no panning past the edges. */
export function clamp(v: View, b: Size): View {
  const s = Math.min(MAX, Math.max(1, v.s));
  const c = (t: number, span: number) => Math.min(0, Math.max(span - span * s, t));
  return { s, tx: c(v.tx, b.w), ty: c(v.ty, b.h) };
}

/** Pinch from `v0` (taken when the two fingers landed at mid `m0`, `d0` apart) to fingers at mid `m`, `d` apart. The point under the fingers stays under them. */
export function pinch(v0: View, m0: Pt, d0: number, m: Pt, d: number, b: Size): View {
  const s = v0.s * (d0 > 0 ? d / d0 : 1);
  const px = (m0.x - v0.tx) / v0.s, py = (m0.y - v0.ty) / v0.s;
  return clamp({ s, tx: m.x - px * s, ty: m.y - py * s }, b);
}

/** Zoom by `k` around box point `p` (double tap). */
export const zoomAt = (v: View, p: Pt, k: number, b: Size) => pinch(v, p, 1, p, k, b);

/**
 * Brings `r` (unzoomed box coordinates) into view while typing: zooms in until its line is
 * about `line` px tall (when `grow`), then pans only if it is off screen, so manual panning sticks.
 */
export function reveal(v: View, r: { x: number; y: number; w: number; h: number }, b: Size, grow: boolean, line = 22): View {
  const s = grow ? Math.max(v.s, Math.min(4, line / Math.max(1, Math.min(r.h, 60)))) : v.s;
  let { tx, ty } = s === v.s ? v : zoomAt(v, { x: r.x * v.s + v.tx, y: r.y * v.s + v.ty }, s / v.s, b);
  const cx = r.x * s + tx, cy = (r.y + Math.min(r.h, 60) / 2) * s + ty, m = 0.15;
  if (cx < b.w * m || cx > b.w * (1 - m)) tx += b.w / 2 - cx;
  if (cy < b.h * m || cy > b.h * (1 - m)) ty += b.h / 2 - cy;
  return clamp({ s, tx, ty }, b);
}

/** One frame of auto-pan while a finger rests within `edge` px of the box border: faster the deeper it goes. */
export function edgePan(v: View, p: Pt, b: Size, edge = 48, speed = 14): View {
  const push = (d: number) => d < edge ? speed * (1 - Math.max(0, d) / edge) : 0;
  return clamp({ s: v.s, tx: v.tx + push(p.x) - push(b.w - p.x), ty: v.ty + push(p.y) - push(b.h - p.y) }, b);
}
