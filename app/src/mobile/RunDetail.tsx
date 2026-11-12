import { useEffect } from "react";
import { loadSession, useStore } from "../api";
import { Markdown } from "../views/chat/Markdown";
import { stopRun } from "./actions";
import { ApproveCard } from "./ApproveCard";
import { STATE_CLASS } from "./RunList";
import { ago, isActive, pendingOf, STATE_TEXT, type Item, type Run } from "./runs";
import { closeRun, openLive, useLabel } from "./store";
import { Back, StepLine } from "./ui";

const PHASE = { plan: "Plan", act: "Act", check: "Check" } as const;

/** Labels the items of one turn: says before the first step are the plan, the steps the acts, later says the check. */
function phased(items: Item[]) {
  let seen = false, last = "";
  return items.map((x) => {
    const p = x.k === "step" ? "act" : seen ? "check" : "plan";
    if (x.k === "step") seen = true;
    const label = p !== last && x.k !== "err" ? PHASE[p] : "";
    if (x.k !== "err") last = p;
    return { x, label };
  });
}

const Label = ({ children }: { children: string }) => <div className="mt-1 mb-1.5 text-[10.5px] font-semibold tracking-[.08em] text-text-3 uppercase">{children}</div>;

export function RunDetail({ run }: { run: Run }) {
  const L = useLabel();
  const watchable = useStore((s) => !!s.devices[run.current]?.connected);
  const pend = pendingOf(run);
  useEffect(() => { loadSession(run.id, run.driver); }, [run.id, run.driver]);

  return (
    <section className="flex min-h-0 flex-1 flex-col" aria-label="Run">
      <header className="flex flex-none items-center gap-1 px-2 py-1">
        <Back onClick={closeRun} label="Back to runs" />
        <div className="min-w-0 flex-1">
          <b className="block truncate text-[15px]">{run.title}</b>
          <small className="text-[11.5px] text-text-2"><span className={STATE_CLASS[run.state]}>{STATE_TEXT[run.state]}</span> Â· {ago(run.updated)}</small>
        </div>
        <span className="flex-none rounded-full border border-line px-2.5 py-1 text-[11px] text-primary">Driver: {L(run.driver)}</span>
      </header>
      <div className="flex flex-none items-center gap-2 border-b border-line-soft px-3.5 pb-2.5 text-[11px] text-text-2">
        <span className="font-semibold text-text">{run.turns.length} pass{run.turns.length === 1 ? "" : "es"}</span>
        <span className="min-w-0 flex-1 truncate">{run.scope.slice(1).map(L).join(" / ") || "Driver only"}</span>
        <button className="m-btn sm" disabled={!watchable} onClick={() => openLive(run.current, { follow: run.id })}>Watch <b className="font-semibold">{L(run.current)}</b></button>
      </div>
      <div className="m-scroll flex-1 px-3.5 pt-3 pb-[calc(var(--sheet-h)+16px)]">
        {run.turns.map((t, i) => (
          <div key={i} className="mb-3.5 ml-0.5 border-l border-line pl-3">
            <b className="mb-2 block text-[11px] font-semibold text-text-2">{i === 0 ? "Plan / Act / Check" : "Follow-up"}</b>
            {t.user && <div className="mb-2 ml-auto w-fit max-w-[88%] rounded-[10px] bg-surface-3 px-3 py-2 break-words whitespace-pre-wrap select-text">{t.user}</div>}
            {phased(t.items).map(({ x, label }, j) => (
              <div key={x.k === "step" ? x.key : j}>
                {label && <Label>{label}</Label>}
                {x.k === "step" ? <StepLine step={x} />
                  : x.k === "say" ? <div className="mb-2 max-w-[92%] rounded-[10px] border border-line-soft bg-surface px-3 py-2 select-text"><Markdown>{x.text}</Markdown></div>
                  : <p className="mb-2 rounded-[10px] border border-bad px-3 py-2 break-words text-bad select-text">{x.text}</p>}
              </div>
            ))}
          </div>
        ))}
        {!run.turns.length && <p className="animate-pulse text-text-3" role="status">Loading the run</p>}
        <div className="flex flex-col gap-2">
          {run.approvals.map((a) => <ApproveCard key={a.request} a={a} device={run.current} />)}
        </div>
        {run.state === "working" && !pend.length && <p className="animate-pulse px-1 py-2 text-text-3" role="status">Working</p>}
        {run.state === "offline" && <p className="py-2 text-text-2">Driver offline. Reconnect {L(run.driver)} to continue this run.</p>}
        {run.state === "failed" && <p className="py-2 text-text-2">Run failed. Send a follow-up below to try again.</p>}
        {isActive(run) && (
          <div className="mt-4 mb-6 flex gap-2">
            <button className="m-btn danger" onClick={() => stopRun(run)}>Stop run</button>
          </div>
        )}
      </div>
    </section>
  );
}
