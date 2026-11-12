import { useEffect, useState, type ReactNode } from "react";
import { Icon } from "../Icon";
import { devColor } from "./devices";
import { openLive, useLabel, useM } from "./store";
import type { Step } from "./runs";

export function Header({ title, sub, children }: { title: string; sub?: string; children?: ReactNode }) {
  return (
    <header className="flex min-h-14 flex-none items-center justify-between gap-2 px-4 pt-1.5 pb-1">
      <div className="min-w-0">
        <h1 className="text-[22px] leading-7 font-semibold tracking-tight">{title}</h1>
        {sub && <p className="truncate text-xs text-text-2">{sub}</p>}
      </div>
      <div className="flex flex-none gap-2">{children}</div>
    </header>
  );
}

export function Empty({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="m-rise mt-2 flex flex-col items-center gap-3 rounded-xl border border-dashed border-line px-5 py-12 text-center text-text-2">
      <b className="text-base font-semibold text-text">{title}</b>
      {children}
    </div>
  );
}

/** Re-renders every `ms`, for "3 min" style labels. */
export function useNow(ms = 20000) {
  const [now, set] = useState(Date.now());
  useEffect(() => { const t = setInterval(() => set(Date.now()), ms); return () => clearInterval(t); }, [ms]);
  return now;
}

/** A device as a small pill in its own color. Tap opens its live view. */
export function DevChip({ name, onClick }: { name: string; onClick?: () => void }) {
  const L = useLabel();
  return (
    <button type="button" className="m-dc" onClick={(e) => { e.stopPropagation(); (onClick ?? (() => openLive(name)))(); }} aria-label={`Watch ${L(name)}`}>
      <i style={{ background: devColor(name) }} />
      <span className="truncate">{L(name)}</span>
    </button>
  );
}

export function StepLine({ step, tag = true }: { step: Step; tag?: boolean }) {
  return (
    <div className={`flex min-h-6 items-center gap-2 px-1 py-0.5 ${step.bad ? "text-bad" : "text-text-2"}`}>
      <i className={`size-[7px] flex-none rounded-full ${step.live ? "animate-pulse" : ""}`} style={{ background: step.bad ? "var(--c-bad)" : devColor(step.device) }} />
      {tag && <DevChip name={step.device} />}
      <span className="mono min-w-0 flex-1 truncate text-[11.5px]" title={step.text}>{step.text}</span>
    </div>
  );
}

export function Toast() {
  const t = useM((m) => m.toast);
  if (!t) return null;
  return (
    <div role="status" key={t.at} className="pointer-events-none fixed top-[calc(env(safe-area-inset-top)+56px)] left-1/2 z-[60] max-w-[calc(100vw-32px)] -translate-x-1/2 truncate rounded-lg border border-line bg-surface-3 px-3.5 py-2 text-[13px] shadow-lg"
      style={{ animation: "m-toast .2s var(--ease)" }}>
      {t.text}
    </div>
  );
}

export const Back = ({ onClick, label = "Back" }: { onClick: () => void; label?: string }) => (
  <button type="button" className="m-btn ghost -ml-2 size-11 !min-h-11 !p-0" onClick={onClick} aria-label={label}><Icon name="back" className="!size-5" /></button>
);
