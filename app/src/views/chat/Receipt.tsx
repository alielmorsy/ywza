import type { SEvent } from "../../api";

type Usage = Extract<SEvent, { type: "usage" }>;

const tokens = (n: number) => n < 1000 ? `${n}` : n < 100_000 ? `${(n / 1000).toFixed(1)}k` : n < 1_000_000 ? `${Math.round(n / 1000)}k` : `${(n / 1e6).toFixed(1)}M`;
const money = (c?: number | null) => c == null ? "" : `, $${c < 0.01 ? c.toFixed(3) : c.toFixed(2)}`;

/** One muted line at the end of a run: what it used and what a screenshot-based agent would have. */
export function Receipt({ u, className = "" }: { u: Usage; className?: string }) {
  if (u.requests === 0) return null;
  return (
    <div className={`${className} text-xs text-text-3 select-text`}>
      This run: {tokens(u.input)} in / {tokens(u.output)} out{money(u.cost)}. A screenshot agent: ~{tokens(u.screenshot_estimate)}{money(u.screenshot_cost).replace(", ", ", ~")}.
    </div>
  );
}
