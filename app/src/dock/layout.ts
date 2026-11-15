import type { Mode } from "./machine";

/** Physical pixels, virtual desktop. */
export interface Rect { x: number; y: number; w: number; h: number }
export interface Work extends Rect { scale: number }
export interface Shape { base: Rect | null; card: Rect | null }

export const union = (a: Rect, b: Rect): Rect => {
  const x = Math.min(a.x, b.x), y = Math.min(a.y, b.y);
  return { x, y, w: Math.max(a.x + a.w, b.x + b.w) - x, h: Math.max(a.y + a.h, b.y + b.h) - y };
};
export const sameRect = (a: Rect | null, b: Rect | null) => a === b || (!!a && !!b && a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h);

const SIZE: Partial<Record<Mode, [number, number]>> = { prompt: [640, 64], working: [420, 40], done: [420, 40], failed: [420, 40], expanded: [560, 520] };
const TAB: [number, number] = [420, 40];
const CARD_W = 480;
/** Tallest approval card, CSS px; taller content scrolls. */
export const CARD_H = 400;
const GAP = 8;

const centered = (work: Work, [w, h]: [number, number], y: number): Rect => {
  const s = work.scale;
  return { x: Math.round(work.x + (work.w - w * s) / 2), y: Math.round(y), w: Math.round(w * s), h: Math.round(h * s) };
};

/** Where the dock draws for `mode`; `approval`: a card hangs under the shape. A hidden dock with an approval shows just the tab and the card. */
export function shape(mode: Mode, approval: boolean, work: Work, cardH = CARD_H): Shape {
  const size = SIZE[mode] ?? (approval ? TAB : undefined);
  if (!size) return { base: null, card: null };
  const base = centered(work, size, mode === "prompt" ? work.y + work.h * 0.22 : work.y);
  const card = approval ? centered(work, [CARD_W, cardH], base.y + base.h + GAP * work.scale) : null;
  return { base, card };
}

export const bounds = ({ base, card }: Shape): Rect | null => (base && card ? union(base, card) : base);
