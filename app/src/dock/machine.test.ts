import test from "node:test";
import assert from "node:assert/strict";
import { step } from "./machine.ts";
import { bounds, shape, union } from "./layout.ts";

test("open, close and send", () => {
  assert.equal(step({ mode: "hidden" }, { t: "hotkey" }).mode, "prompt");
  assert.equal(step({ mode: "prompt" }, { t: "esc" }).mode, "hidden");
  assert.deepEqual(step({ mode: "prompt" }, { t: "submit", session: "a" }), { mode: "working", session: "a" });
});

test("approval_from_hidden_returns_to_hidden", () => {
  const a = step({ mode: "hidden" }, { t: "approval", request: 7 });
  assert.equal(a.approval, 7);
  assert.deepEqual(step(a, { t: "resolved", request: 7 }), { mode: "hidden" });
  assert.equal(step(a, { t: "resolved", request: 8 }).approval, 7);
});

test("approval survives the prompt and the working tab", () => {
  const s = step({ mode: "prompt", approval: 3 }, { t: "submit", session: "a" });
  assert.deepEqual(s, { mode: "working", session: "a", approval: 3 });
});

test("finishing", () => {
  assert.equal(step({ mode: "working", session: "a" }, { t: "done" }).mode, "done");
  assert.equal(step({ mode: "done", session: "a" }, { t: "timeout" }).mode, "hidden");
  assert.equal(step({ mode: "failed", session: "a" }, { t: "timeout" }).mode, "failed");
  assert.equal(step({ mode: "failed", session: "a" }, { t: "esc" }).mode, "hidden");
});

test("stop_hides_the_tab", () => {
  const hidden = step({ mode: "working", session: "a" }, { t: "cancel" });
  assert.equal(hidden.mode, "hidden");
  assert.equal(step(hidden, { t: "failed" }).mode, "hidden");
});

test("expanding", () => {
  assert.equal(step({ mode: "working", session: "a" }, { t: "click_tab" }).mode, "expanded");
  assert.equal(step({ mode: "working", session: "a" }, { t: "hotkey" }).mode, "expanded");
});

test("closing the panel goes back to the tab while running", () => {
  const open = { mode: "expanded" as const, session: "a" };
  assert.equal(step(open, { t: "esc", running: true }).mode, "working");
  assert.equal(step(open, { t: "esc", running: false }).mode, "hidden");
  assert.equal(step(open, { t: "blur", running: true }).mode, "working");
  assert.equal(step(open, { t: "blur", running: false }).mode, "hidden");
});

test("clicking outside closes the prompt but not the tab", () => {
  assert.equal(step({ mode: "prompt" }, { t: "blur", running: false }).mode, "hidden");
  assert.equal(step({ mode: "working", session: "a" }, { t: "blur", running: true }).mode, "working");
});

test("geometry scales with the monitor", () => {
  const work = { x: 100, y: 50, w: 2000, h: 1000, scale: 2 };
  assert.deepEqual(shape("prompt", false, work).base, { x: 100 + (2000 - 1280) / 2, y: 50 + 220, w: 1280, h: 128 });
  assert.deepEqual(shape("working", false, work).base, { x: 100 + (2000 - 840) / 2, y: 50, w: 840, h: 80 });
  assert.equal(bounds(shape("hidden", false, work)), null);
  assert.equal(shape("hidden", true, work).card?.y, 50 + 80 + 16);
  assert.deepEqual(union({ x: 0, y: 0, w: 10, h: 10 }, { x: 5, y: 5, w: 10, h: 10 }), { x: 0, y: 0, w: 15, h: 15 });
});

test("opening with context", () => {
  const context = { source: "clipboard" as const, items: [{ kind: "text" as const, text: "boom" }] };
  const open = step({ mode: "hidden" }, { t: "open", context });
  assert.deepEqual(open, { mode: "prompt", context });
  assert.equal(step({ mode: "done", session: "a" }, { t: "open", context }).context, context);
  assert.deepEqual(step({ mode: "working", session: "a" }, { t: "open", context }), { mode: "working", session: "a" });
  assert.deepEqual(step({ mode: "expanded", session: "a" }, { t: "open", context }), { mode: "expanded", session: "a" });
  assert.deepEqual(step(open, { t: "esc" }), { mode: "hidden" });
  assert.deepEqual(step(open, { t: "blur", running: false }), { mode: "hidden" });
  assert.deepEqual(step(open, { t: "submit", session: "b" }), { mode: "working", session: "b" });
  assert.equal(step(open, { t: "approval", request: 1 }).context, context);
  assert.equal(step({ mode: "hidden" }, { t: "hotkey" }).context, undefined);
});

test("context grows the prompt", () => {
  const work = { x: 0, y: 0, w: 1000, h: 1000, scale: 1 };
  assert.equal(shape("prompt", false, work, undefined, 150).base?.h, 150);
  assert.equal(shape("working", false, work, undefined, 150).base?.h, 40);
});
