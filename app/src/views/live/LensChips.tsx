import { useEffect, useState, type RefObject } from "react";
import type { LensElement, Monitor } from "../../api";
import { chipName, toBox, type LensCtl } from "./lens";

const MIN_PX = 16;

/** Tappable boxes over the streamed screen. `scale` is the zoom of the parent, so labels and hit areas keep their on-screen size. */
export function LensLayer({ lens, canvas, mon, scale = 1 }: { lens: LensCtl; canvas: RefObject<HTMLCanvasElement | null>; mon?: Monitor; scale?: number }) {
  const [, redraw] = useState(0);
  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const ro = new ResizeObserver(() => redraw((n) => n + 1));
    ro.observe(c);
    return () => ro.disconnect();
  }, [canvas]);
  const c = canvas.current;
  if (!lens.active || !c || !mon) return null;

  const items = [...lens.shown].sort((a, b) => b.rect.width * b.rect.height - a.rect.width * a.rect.height); // small ones on top
  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden" role="group" aria-label="Elements of the followed window">
      {items.map((el) => {
        const b = toBox(c, mon, el.rect);
        if (!b) return null;
        const w = Math.max(b.w, MIN_PX / scale), h = Math.max(b.h, MIN_PX / scale);
        const hit = lens.filtering;
        return (
          <button key={el.id} type="button" aria-disabled={!lens.canAct} title={`${chipName(el)} (${el.role})${el.editable ? ", editable" : ""}`} aria-label={`${chipName(el)}, ${el.role}`}
            onClick={() => lens.tap(el)}
            className={`pointer-events-auto absolute rounded-[3px] border p-0 text-left ${el.editable ? "border-dashed" : ""} ${hit ? "z-10 border-accent-bg bg-accent-soft" : "border-accent/70 bg-accent/5"} ${lens.canAct ? "" : "cursor-default opacity-60"}`}
            style={{ left: b.x, top: b.y, width: w, height: h, borderWidth: 1 / scale }}>
            {el.name && w * scale >= 44 && (
              <span className="mono pointer-events-none absolute top-0 left-0 block origin-top-left truncate bg-accent px-[3px] text-[10px] leading-[14px] text-ink"
                style={{ width: w * scale, transform: `scale(${1 / scale})` }}>{el.name}</span>
            )}
          </button>
        );
      })}
    </div>
  );
}

/** Text for an editable element: Enter types it and presses Enter; "Type" only replaces the content. */
export function LensEditor({ el, onType, onClose, mobile, className = "" }: { el: LensElement; onType: (text: string, submit: boolean) => Promise<void>; onClose: () => void; mobile?: boolean; className?: string }) {
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const btn = mobile ? "m-btn sm flex-1" : "btn flex-1 justify-center";
  const go = async (submit: boolean) => { setBusy(true); await onType(text, submit); setBusy(false); };
  return (
    <form className={`flex flex-col gap-2 rounded-lg border border-line bg-chrome p-2.5 shadow-lg ${className}`} aria-label={`Type into ${chipName(el)}`}
      onSubmit={(e) => { e.preventDefault(); if (!busy) go(true); }} onKeyDown={(e) => { if (e.key === "Escape") onClose(); }}>
      <label className="flex flex-col gap-1">
        <span className="truncate text-xs text-text-2">Type into {chipName(el)}</span>
        <input autoFocus value={text} autoComplete="off" onChange={(e) => setText(e.target.value)}
          className={mobile ? "h-10 rounded-md border border-line bg-ground px-2.5 outline-none focus:border-accent focus:shadow-[0_0_0_3px_var(--c-accent-soft)]" : "field"} />
      </label>
      <div className="flex gap-2">
        <button type="button" className={btn} onClick={onClose}>Cancel</button>
        <button type="button" className={btn} disabled={busy} onClick={() => go(false)}>Type</button>
        <button type="submit" className={`${btn} primary`} disabled={busy}>Type and Enter</button>
      </div>
    </form>
  );
}
