import { useRef, useState, type KeyboardEvent, type PointerEvent, type ReactNode } from "react";

const KEY = "ywza.live.dock", MIN = 280, MAX = 480;
const clamp = (n: number) => Math.min(MAX, Math.max(MIN, Math.round(n)));
const load = () => { try { return clamp(Number(localStorage.getItem(KEY)) || 340); } catch { return 340; } };

/** Right-hand panel with a drag handle on its left edge. */
export function DockPanel({ children }: { children: ReactNode }) {
  const [w, setW] = useState(load);
  const panel = useRef<HTMLElement>(null);
  const set = (n: number) => {
    const v = clamp(n);
    setW(v);
    try { localStorage.setItem(KEY, String(v)); } catch { /* private mode */ }
  };
  const drag = (e: PointerEvent<HTMLDivElement>) => {
    if (e.buttons === 1 && panel.current) set(panel.current.getBoundingClientRect().right - e.clientX);
  };
  const key = (e: KeyboardEvent) => {
    const d = e.key === "ArrowLeft" ? 16 : e.key === "ArrowRight" ? -16 : 0;
    if (d) { e.preventDefault(); set(w + d); }
  };

  return (
    <aside ref={panel} className="relative flex min-h-0 min-w-0 max-w-[55%] flex-none flex-col border-l border-line bg-ground" style={{ width: w }}>
      <div role="separator" aria-orientation="vertical" aria-label="Resize chat" aria-valuemin={MIN} aria-valuemax={MAX} aria-valuenow={w} tabIndex={0}
        className="group absolute inset-y-0 -left-1 z-10 w-2 cursor-col-resize touch-none"
        onPointerDown={(e) => e.currentTarget.setPointerCapture(e.pointerId)} onPointerMove={drag} onKeyDown={key}>
        <span className="absolute inset-y-0 left-[3px] w-px bg-transparent transition-colors group-hover:bg-accent group-focus-visible:bg-accent" />
      </div>
      <div className="flex min-h-0 flex-1 flex-col">{children}</div>
    </aside>
  );
}
