import { useMemo } from "react";
import { helpOf, useStore } from "../api";
import { Icon } from "../Icon";
import { ApproveCard } from "./ApproveCard";
import { HelpCard } from "./HelpCard";
import { FiredCard, useFired } from "./Fired";
import { useRuns } from "./runs";
import { openRun, useLabel } from "./store";
import { Empty, Header } from "./ui";

/** Unresolved approvals, runs waiting for help and fired watchers, for the tab badge. */
export const usePendingCount = () => useStore((s) =>
  Object.values(s.remoteApprovals).filter((a) => !a.resolved).length + Object.values(s.remote).filter((x) => helpOf(s.events[x.id])).length + Object.values(s.devices).reduce((n, d) => n + (d.fired?.length ?? 0), 0));

export function Inbox() {
  const L = useLabel();
  const runs = useRuns();
  const apps = useStore((s) => s.remoteApprovals);
  const fired = useFired();
  const { waiting, done } = useMemo(() => {
    const all = Object.values(apps);
    return { waiting: all.filter((a) => !a.resolved).reverse(), done: all.filter((a) => a.resolved).slice(-6).reverse() };
  }, [apps]);
  const run = (id: string, driver?: string) => runs.find((r) => r.id === id && r.driver === driver);
  const helping = runs.filter((r) => r.help);

  return (
    <section className="flex min-h-0 flex-1 flex-col" aria-label="Inbox">
      <Header title="Inbox" sub={`${waiting.length + helping.length + fired.length} waiting`} />
      <div className="m-scroll flex flex-1 flex-col gap-2 px-3.5 pt-1 pb-4">
        {helping.map((r) => <HelpCard key={`${r.driver}:${r.id}`} run={r} title={r.title} />)}
        {fired.map((f) => <FiredCard key={`${f.device}:${f.watcher}`} f={f} />)}
        {waiting.map((a) => {
          const r = run(a.session, a.device);
          return <ApproveCard key={`${a.device}:${a.request}`} a={a} device={r?.current ?? a.device} run={r?.title} />;
        })}
        {!waiting.length && !helping.length && !fired.length && (
          <Empty title="All clear">
            <Icon name="check" className="!size-6" />
            <p>Nothing needs you right now.</p>
          </Empty>
        )}
        {done.length > 0 && (
          <>
            <h2 className="mt-4 mb-1 px-0.5 text-[11px] font-semibold tracking-[.08em] text-text-2 uppercase">Resolved</h2>
            {done.map((a) => {
              const r = run(a.session, a.device);
              return (
                <button key={`${a.device}:${a.request}`} type="button" disabled={!r} onClick={() => r && openRun(r.id)} className="rounded-xl border border-line-soft bg-surface px-3 py-2.5 text-left opacity-75">
                  <div className="flex justify-between gap-2 text-[13px]"><b className="truncate font-semibold">{L(r?.current ?? a.device ?? "")}</b><span className="flex-none text-xs text-text-2">{a.allowed ? "Allowed" : "Denied"}</span></div>
                  <div className="mono mt-0.5 truncate text-xs text-text-2">{a.tool} {a.summary}</div>
                  {r && <div className="mt-0.5 truncate text-xs text-text-3">{r.title}</div>}
                </button>
              );
            })}
          </>
        )}
      </div>
    </section>
  );
}
