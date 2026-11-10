import type { KeyboardEvent } from "react";

export const BUTTONS = ["left", "middle", "right"] as const; // PointerEvent.button order

/** Part of the monitor a canvas shows, 0..1. */
export interface Region { x: number; y: number; w: number; h: number }
export const FULL: Region = { x: 0, y: 0, w: 1, h: 1 };

const NAMED: Record<string, string> = {
  Control: "ctrl", Shift: "shift", Alt: "alt", Meta: "win", Enter: "enter", Escape: "esc", Tab: "tab",
  " ": "space", Backspace: "backspace", Delete: "delete", Insert: "insert", Home: "home", End: "end",
  PageUp: "pageup", PageDown: "pagedown", ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left",
  ArrowRight: "right", CapsLock: "capslock", PrintScreen: "printscreen", ContextMenu: "menu", "+": "plus", "-": "minus",
};

/** `press_keys` name for a key event. Letters and digits go by physical key so shift doesn't change them. */
export function keyName(e: KeyboardEvent): string | null {
  if (NAMED[e.key]) return NAMED[e.key];
  if (/^F\d{1,2}$/.test(e.key)) return e.key.toLowerCase();
  const m = /^(?:Key|Digit)(.)$/.exec(e.code);
  if (m) return m[1].toLowerCase();
  return e.key.length === 1 ? e.key.toLowerCase() : null;
}

/** Pointer position as monitor coordinates (0..1, clamped). The canvas is letterboxed by object-contain; `inside` says whether the pointer is on the picture. */
export function pointOn(c: HTMLCanvasElement, e: { clientX: number; clientY: number }, r: Region) {
  if (!c.width || !c.height) return null;
  const b = c.getBoundingClientRect(), s = Math.min(b.width / c.width, b.height / c.height);
  const w = c.width * s, h = c.height * s;
  const u = (e.clientX - b.left - (b.width - w) / 2) / w, v = (e.clientY - b.top - (b.height - h) / 2) / h;
  const cl = (n: number) => Math.min(1, Math.max(0, n));
  return { x: r.x + cl(u) * r.w, y: r.y + cl(v) * r.h, inside: u >= 0 && u <= 1 && v >= 0 && v <= 1 };
}

/** The agent wants notches (+ = down/right); a pixel-mode notch is about 100px. */
export function wheelNotches(e: { deltaMode: number; deltaX: number; deltaY: number }) {
  const k = e.deltaMode === 1 ? 1 / 3 : e.deltaMode === 2 ? 1 : 1 / 100;
  return { dx: e.deltaX * k, dy: e.deltaY * k };
}
