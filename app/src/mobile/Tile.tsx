import { useRef, type PointerEvent } from "react";
import { useStore } from "../api";
import { Icon } from "../Icon";
import { DEV_COLOR, DEV_TEXT, type TileInfo } from "./tiles";
import { openRun, setMenu, toggleSel, useLabel, useM } from "./store";

const HOLD_MS = 500;

export function Tile({ name, info, onOpen }: { name: string; info: TileInfo; onOpen: (name: string, from: DOMRect) => void }) {
  const L = useLabel();
  const shot = useStore((s) => s.shots[name]);
  const selected = useM((m) => m.sel.includes(name));
  const hold = useRef<{ t: ReturnType<typeof setTimeout>; x: number; y: number; fired: boolean }>(undefined);
  const screen = useRef<HTMLButtonElement>(null);
  const off = info.state === "offline";

  const stop = () => { if (hold.current) clearTimeout(hold.current.t); };
  const down = (e: PointerEvent) => {
    stop();
    hold.current = { x: e.clientX, y: e.clientY, fired: false, t: setTimeout(() => { hold.current!.fired = true; navigator.vibrate?.(10); setMenu(name); }, HOLD_MS) };
  };
  const move = (e: PointerEvent) => { const h = hold.current; if (h && Math.hypot(e.clientX - h.x, e.clientY - h.y) > 10) stop(); };
  const click = () => { if (hold.current?.fired) { hold.current.fired = false; return; } if (!off && screen.current) onOpen(name, screen.current.getBoundingClientRect()); };

  return (
    <div className="m-tile" data-st={info.state} data-sel={selected} data-tile={name}>
      <div className="relative">
        <button ref={screen} type="button" aria-label={off ? `${L(name)} is offline` : `Watch ${L(name)} live`} aria-disabled={off}
          className="relative block aspect-[16/10] w-full overflow-hidden bg-chrome" style={{ touchAction: "pan-y" }}
          onPointerDown={down} onPointerMove={move} onPointerUp={stop} onPointerLeave={stop} onPointerCancel={stop} onClick={click}>
          {shot && !off
            ? <img src={shot.src} alt="" draggable={false} className="size-full object-cover" />
            : <span className="absolute inset-0 grid place-items-center text-text-3"><Icon name={off ? "unlink" : "monitor"} className="!size-6" /></span>}
          {info.here && <span className="absolute top-2 left-2 rounded-full bg-accent px-2 py-0.5 text-[10px] font-bold text-ink">AI here</span>}
        </button>
        <button type="button" role="checkbox" aria-checked={selected} aria-label={`Let the assistant use ${L(name)}`}
          className="absolute top-0 right-0 grid size-11 place-items-center" onClick={() => toggleSel(name)}>
          <span className={`grid size-6 place-items-center rounded-full border-[1.5px] transition-colors ${selected ? "border-primary bg-primary text-ink" : "border-text-2 bg-chrome/70"}`}>
            {selected && <Icon name="check" className="!size-3.5 !stroke-[2.4]" />}
          </span>
        </button>
      </div>
      <div className="px-2.5 pt-1.5 pb-2">
        <div className="flex items-center justify-between gap-2 text-[13px] font-semibold">
          <span className="truncate">{L(name)}</span>
          <span className="flex-none text-[11px] font-normal" style={{ color: DEV_COLOR[info.state] }}>{DEV_TEXT[info.state]}</span>
        </div>
        <div className="mono mt-0.5 truncate text-[11px]" style={{ color: info.state === "need" ? "var(--c-warn)" : "var(--c-text-2)" }}>{info.ticker}</div>
        {info.run && (
          <button type="button" className="relative mt-1.5 flex h-6 max-w-full items-center gap-1.5 truncate rounded-full border border-line bg-surface-2 px-2.5 text-[11px] after:absolute after:-inset-y-2.5 after:inset-x-0"
            onClick={() => openRun(info.run!.id)} aria-label={`Open run ${info.run.title}`}>
            <i className="size-[7px] flex-none rounded-full" style={{ background: DEV_COLOR[info.state] }} /><span className="truncate">{info.run.title}</span>
          </button>
        )}
      </div>
    </div>
  );
}
