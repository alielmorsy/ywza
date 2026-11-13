import { devColor } from "./devices";
import { devicesOf, elapsed, isActive, STATE_TEXT, useRuns, ago, type Run, type RunState } from "./runs";
import { openRun, setTab, useLabel } from "./store";
import { Empty, Header, useNow } from "./ui";

export const STATE_CLASS: Record<RunState, string> = { working: "text-accent", needs: "text-warn", done: "text-text", stopped: "text-text-2", failed: "text-bad", offline: "text-text-2" };

function RunCard({ r }: { r: Run }) {
  const L = useLabel();
  useNow();
  const trace = r.steps.slice(-12);
  return (
    <button type="button" onClick={() => openRun(r.id)} className="m-rise w-full rounded-xl border border-line-soft bg-surface p-3 text-left active:bg-surface-2">
      <div className="flex items-baseline justify-between gap-2">
        <b className="min-w-0 truncate text-[15px] font-semibold">{r.title}</b>
        <small className="flex-none text-[11.5px] text-text-2">{ago(r.updated)}</small>
      </div>
      <div className="mt-1 flex items-center gap-2 text-xs text-text-2">
        <span className="flex-none rounded-full border border-line px-2 py-0.5 text-[11px] text-primary">Driver: {L(r.driver)}</span>
        <span className="min-w-0 truncate">{devicesOf(r).slice(1).map(L).join(" / ") || "Driver only"}</span>
      </div>
      <div className="mt-2 flex gap-3 text-[11px] text-text-2">
        <span className={`font-semibold ${STATE_CLASS[r.state]}`}>{STATE_TEXT[r.state]}</span>
        <span>{r.steps.length} step{r.steps.length === 1 ? "" : "s"}</span>
        <span>{isActive(r) ? elapsed(r) : ago(r.updated)}</span>
      </div>
      <div className="mt-2 flex h-3 items-center gap-1" aria-hidden>
        {trace.map((s) => <i key={s.key} className="h-[5px] w-[9px] flex-none rounded-full" style={{ background: s.bad ? "var(--c-bad)" : devColor(s.device) }} />)}
        {!trace.length && <i className="h-[5px] w-[9px] animate-pulse rounded-full bg-accent" />}
      </div>
      <p className="mono mt-1.5 truncate text-[12px] text-text-2">{r.last || (isActive(r) ? "Planning the first step" : "No steps")}</p>
    </button>
  );
}

export function Runs() {
  const runs = useRuns();
  const active = runs.filter(isActive).length;
  return (
    <section className="flex min-h-0 flex-1 flex-col" aria-label="Runs">
      <Header title="Runs" sub={runs.length ? `${active} active · ${runs.length} total` : "What your assistants did"}>
        <button className="m-btn primary sm" onClick={() => setTab("wall")}>New run</button>
      </Header>
      <div className="m-scroll flex flex-1 flex-col gap-2 px-3.5 pt-1 pb-4">
        {runs.map((r) => <RunCard key={r.id} r={r} />)}
        {!runs.length && (
          <Empty title="No runs yet">
            <p className="max-w-[270px]">Describe a task on the Wall. The driver plans it, acts on your devices and checks the result.</p>
            <button className="m-btn primary" onClick={() => setTab("wall")}>Start a run</button>
          </Empty>
        )}
      </div>
    </section>
  );
}
