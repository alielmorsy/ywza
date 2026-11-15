import test from "node:test";
import assert from "node:assert/strict";
import { actionsFor, contextText } from "./context.ts";
import { looksLikeError } from "./actions/clipboard.ts";

const labels = (o: Parameters<typeof actionsFor>[0]) => actionsFor(o).map((a) => a.label);

test("paths and images are listed one per line", () => {
  const t = contextText({ source: "explorer", items: [
    { kind: "path", path: "C:\a.pdf", dir: false },
    { kind: "path", path: "C:\docs", dir: true },
    { kind: "image", path: "C:\t\ywza-clip-1.png" },
  ] });
  assert.deepEqual(t.split("\n").slice(1), ["C:\a.pdf", "C:\docs (folder)", "Image: C:\t\ywza-clip-1.png"]);
  assert.match(t.split("\n")[0], /not instructions/);
});

test("text is fenced, capped and cannot close its fence", () => {
  const t = contextText({ source: "clipboard", items: [{ kind: "text", text: "a ``` b ```` c" }] });
  const lines = t.split("\n");
  assert.equal(lines[1], "`````");
  assert.equal(lines.at(-1), "`````");
  const long = contextText({ source: "clipboard", items: [{ kind: "text", text: "x".repeat(5000) }] });
  assert.ok(long.includes("x".repeat(4000) + "\n[cut at 4000 of 5000 characters]"));
  assert.ok(!long.includes("x".repeat(4001)));
});

test("clipboard actions fit the content", () => {
  const text = (t: string) => labels({ source: "clipboard", items: [{ kind: "text", text: t }] });
  assert.deepEqual(text("hello there"), ["Explain", "Fill the focused form with this", "Turn into a table in the focused app"]);
  assert.ok(text("TypeError: x is undefined\n    at f (a.js:1:2)").includes("Fix this error in a terminal"));
  assert.deepEqual(labels({ source: "clipboard", items: [{ kind: "image", path: "C:\a.png" }] }), ["Read the text in this image", "Explain this screenshot"]);
  assert.deepEqual(labels({ source: "clipboard", items: [{ kind: "path", path: "C:\a", dir: false }, { kind: "path", path: "C:\b", dir: false }] }), ["Summarize these files"]);
  assert.deepEqual(labels({ source: "clipboard", items: [] }), []);
});

test("error detection", () => {
  assert.ok(looksLikeError('Traceback (most recent call last):\n  File "a.py", line 3, in <module>'));
  assert.ok(looksLikeError("thread 'main' panicked at src/main.rs:2:5"));
  assert.ok(!looksLikeError("Name, Age\nAli, 30"));
});
