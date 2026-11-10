import { useRef, type RefObject } from "react";

interface Props { label?: string; left: RefObject<HTMLElement | null>; onSize: (px: number) => void; onCommit: () => void; value?: number; min: number; max: number }

export function Splitter({ label = "Resize chat and window", left, onSize, onCommit, value, min, max }: Props) {
  const drag = useRef<{ x: number; w: number } | null>(null);
  const width = () => left.current?.offsetWidth ?? 0;
  return (
    <div role="separator" aria-orientation="vertical" aria-valuenow={value} aria-valuemin={min} aria-valuemax={max} aria-label={label} tabIndex={0}
      className="group flex w-2 flex-none cursor-col-resize touch-none justify-center"
      onPointerDown={(e) => { e.currentTarget.setPointerCapture(e.pointerId); drag.current = { x: e.clientX, w: width() }; }}
      onPointerMove={(e) => { if (drag.current) onSize(drag.current.w + e.clientX - drag.current.x); }}
      onLostPointerCapture={() => { drag.current = null; }}
      onPointerUp={() => { drag.current = null; onCommit(); }}
      onKeyDown={(e) => {
        if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
        e.preventDefault();
        onSize(width() + (e.key === "ArrowLeft" ? -24 : 24));
        onCommit();
      }}>
      <div className="h-full w-px bg-line-soft transition-colors group-hover:bg-accent group-focus-visible:bg-accent" />
    </div>
  );
}
