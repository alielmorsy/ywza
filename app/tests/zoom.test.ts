// node --test app/tests
import assert from "node:assert/strict";
import { test } from "node:test";
import { clamp, edgePan, NONE, pinch, reveal } from "../src/mobile/live/zoom.ts";

const box = { w: 400, h: 250 };

test("clamp keeps the picture covering the box", () => {
  assert.deepEqual(clamp({ s: 0.5, tx: 30, ty: 30 }, box), NONE);
  assert.deepEqual(clamp({ s: 2, tx: -1000, ty: 10 }, box), { s: 2, tx: -400, ty: 0 });
});

test("pinch keeps the point under the fingers", () => {
  const v = pinch(NONE, { x: 100, y: 100 }, 50, { x: 100, y: 100 }, 100, box);
  assert.equal(v.s, 2);
  assert.equal((100 - v.tx) / v.s, 100); // content point 100 still under x=100
});

test("reveal zooms to a readable line and centers an off-screen caret", () => {
  const v = reveal(NONE, { x: 390, y: 240, w: 2, h: 5 }, box, true);
  assert.equal(v.s, 4);
  const cx = 390 * v.s + v.tx, cy = 242.5 * v.s + v.ty;
  assert.ok(cx > 0 && cx < box.w && cy > 0 && cy < box.h, `${cx},${cy} on screen`);
  const same = reveal(v, { x: 390, y: 240, w: 2, h: 5 }, box, false);
  assert.deepEqual(same, v, "a visible caret does not move the view");
});

test("edgePan scrolls toward the edge the finger rests on", () => {
  const v = { s: 2, tx: -200, ty: -100 };
  const left = edgePan(v, { x: 0, y: 125 }, box);
  assert.ok(left.tx > v.tx && left.ty === v.ty);
  const corner = edgePan(v, { x: 400, y: 250 }, box);
  assert.ok(corner.tx < v.tx && corner.ty < v.ty);
  assert.deepEqual(edgePan(v, { x: 200, y: 125 }, box), v); // middle: no pan
  assert.deepEqual(edgePan(NONE, { x: 0, y: 0 }, box), NONE); // unzoomed: nothing to pan
});
