import { useEffect, useState, type ReactNode } from "react";

export const Section = ({ h, children }: { h: string; children: ReactNode }) => (
  <section id={`settings-${h.toLowerCase().replaceAll(" ", "-")}`} className="scroll-mt-3">
    <h2 className="mb-1.5">{h}</h2>
    <div className="rounded-md border border-line bg-surface">{children}</div>
  </section>
);

/** Label and hint on the left, control on the right. A wrapping label makes the whole row click through to the control.
    `err` replaces the hint while the value is invalid. */
export const Row = ({ t, d, err, w = "w-[320px]", children }: { t: string; d?: string; err?: string | null; w?: string; children: ReactNode }) => (
  <label className="form-row flex min-h-[44px] items-center gap-4 border-t border-line-soft px-3.5 py-2 first:border-t-0">
    <span className="min-w-0 flex-1">
      <b className="block font-medium">{t}</b>
      {err ? <small className="block text-xs text-bad">{err}</small> : d && <small className="block text-xs text-text-2">{d}</small>}
    </span>
    <span className={`form-control flex min-w-0 flex-none items-center justify-end gap-1.5 ${w}`}>{children}</span>
  </label>
);

export const Tog = ({ on, set, disabled }: { on: boolean; set: (v: boolean) => void; disabled?: boolean }) => (
  <button type="button" className="tog disabled:opacity-40" role="switch" aria-checked={on} disabled={disabled} onClick={() => set(!on)} />
);

/** Integer field that lets the text be empty while typing; the value is clamped to min..max. `unit` shows after it. */
export function Num({ value, onChange, min = 0, max = 1_000_000, unit }: { value: number; onChange: (v: number) => void; min?: number; max?: number; unit?: string }) {
  const [text, setText] = useState(String(value));
  const parse = (t: string) => Math.min(max, Math.max(min, Number(t) || 0));
  useEffect(() => { if (parse(text) !== value) setText(String(value)); }, [value]);
  const input = (
    <input
      className="field mono"
      inputMode="numeric"
      value={text}
      onChange={(e) => { const t = e.target.value.replace(/\D/g, ""); setText(t); onChange(parse(t)); }}
      onBlur={() => setText(String(value))}
    />
  );
  return unit ? <>{input}<span className="w-10 flex-none text-xs text-text-2">{unit}</span></> : input;
}
