// Wall thumbnails: one screenshot per wanted device every 3 s, only while the app is in the foreground.
import { useEffect } from "react";
import { getState, send } from "../api";

const want = new Map<string, number>();
const asked = new Map<string, number>();
const listed = new Set<string>();
let timer: ReturnType<typeof setInterval> | undefined;

function tick() {
  if (document.hidden) return;
  const s = getState();
  for (const n of want.keys()) {
    const d = s.devices[n];
    if (!d?.connected || d.permissions?.view === false) continue;
    if (!d.monitors && !d.outdated && !listed.has(n)) { listed.add(n); send(n, { type: "list_monitors" }); }
    const last = asked.get(n) ?? 0;
    if (last > (s.shots[n]?.at ?? 0) && Date.now() - last < 8000) continue; // the last one is still on its way
    asked.set(n, Date.now());
    send(n, { type: "screenshot", target: { type: "monitor", id: d.monitors?.find((m) => m.primary)?.index ?? 0 }, max_width: 480 });
  }
}

/** Keeps the thumbnails of `names` fresh while `on`. */
export function useThumbs(names: string[], on = true) {
  const key = names.join("\n");
  useEffect(() => {
    if (!on || !key) return;
    const list = key.split("\n");
    list.forEach((n) => want.set(n, (want.get(n) ?? 0) + 1));
    tick();
    timer ??= setInterval(tick, 3000);
    const visible = () => { if (!document.hidden) tick(); };
    document.addEventListener("visibilitychange", visible);
    return () => {
      document.removeEventListener("visibilitychange", visible);
      list.forEach((n) => { const c = (want.get(n) ?? 1) - 1; if (c) want.set(n, c); else want.delete(n); });
      if (!want.size) { clearInterval(timer); timer = undefined; }
    };
  }, [key, on]);
}
