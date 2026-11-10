import type { Rect } from "../../api";

const resize = (c: HTMLCanvasElement, w: number, h: number) => {
  if (c.width !== w || c.height !== h) { c.width = w; c.height = h; }
};

export function drawFull(c: HTMLCanvasElement, f: VideoFrame) {
  resize(c, f.displayWidth, f.displayHeight);
  c.getContext("2d")!.drawImage(f, 0, 0);
}

/** Draws the part of `f` under `r` (monitor pixels, times `k` for the frame scale). The canvas covers all of `r`, off-screen parts stay blank. */
export function drawCrop(c: HTMLCanvasElement, f: VideoFrame, r: Rect, k: number) {
  const x = r.x * k, y = r.y * k;
  resize(c, Math.max(1, Math.round(r.width * k)), Math.max(1, Math.round(r.height * k)));
  const ctx = c.getContext("2d")!;
  ctx.clearRect(0, 0, c.width, c.height);
  const sx = Math.max(0, x), sy = Math.max(0, y);
  const sw = Math.min(f.displayWidth, x + c.width) - sx, sh = Math.min(f.displayHeight, y + c.height) - sy;
  if (sw > 0 && sh > 0) ctx.drawImage(f, sx, sy, sw, sh, sx - x, sy - y, sw, sh);
}
