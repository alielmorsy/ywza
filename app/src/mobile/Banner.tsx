import { useMemo, useState } from "react";
import { useStore } from "../api";
import { Icon } from "../Icon";
import { isActive, useRuns } from "./runs";
import { openLive, useLabel, useM } from "./store";
import { DEV_COLOR } from "./tiles";
import { useThumbs } from "./thumbs";

/** One line for everything the assistants are doing; expands into live thumbnails of the devices being acted on. */
export function Banner() {
  const L = useLabel();
  const runs = useRuns();
  const [open, setOpen] = useState(false);
  const shots = useStore((s) => s.shots);
  const live = useM((m) => !!m.live);
  const active = runs.filter(isActive);
  const acting = useMemo(() => [...new Set(active.map((r) => r.current))].slice(0, 4), [active]);
  useThumbs(acting, open && !live);
  if (!active.length) return null;

  const needs = active.filter((r) => r.state === "needs");
  const text = active.length === 1
    ? `${active[0].title}${needs.length ? " · needs you" : ""}`
    : `${active.length} runs · ${active.length - needs.length} working${needs.length ? ` · ${needs.length} need you` : ""}`;
  return (
    <div className={`m-rise mx-3 mb-2 flex-none overflow-hidden rounded-xl border bg-surface-2 ${needs.length ? "border-warn/50" : "border-line"}`}>
      <button type="button" className="flex min-h-11 w-full items-center gap-2.5 px-3 text-left text-[12.5px]" aria-expanded={open} onClick={() => setOpen(!open)}>
        <i className={`size-2 flex-none animate-pulse rounded-full ${needs.length ? "bg-warn" : "bg-accent"}`} />
        <span className="min-w-0 flex-1 truncate">{text}</span>
        <Icon name="chev" className={`text-text-2 transition-transform ${open ? "rotate-180" : ""}`} />
      </button>
      {open && (
        <div className="grid grid-cols-2 gap-2 px-2.5 pb-2.5">
          {acting.map((n) => (
            <button key={n} type="button" className="relative block aspect-[16/10] overflow-hidden rounded-md border border-line bg-chrome" aria-label={`Watch ${L(n)} live`} onClick={() => openLive(n)}>
              {shots[n] && <img src={shots[n].src} alt="" className="size-full object-cover" />}
              <span className="absolute bottom-1 left-1 flex items-center gap-1.5 rounded-md bg-chrome/85 px-1.5 py-0.5 text-[10px]">
                <i className="size-[7px] rounded-full" style={{ background: DEV_COLOR[active.find((r) => r.current === n)?.state === "needs" ? "need" : "working"] }} />{L(n)}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
