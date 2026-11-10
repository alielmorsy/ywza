import { useMemo } from "react";
import { Icon } from "../../Icon";
import { getState, loadSession, sessionsOf, useStore } from "../../api";
import { setUi, uiOf, useNav } from "../../nav";
import { ago, label } from "./util";

export function SessionList({ dev }: { dev: string }) {
  const sessions = useStore((s) => s.sessions);
  const list = useMemo(() => sessionsOf(getState(), dev), [sessions, dev]); // eslint-disable-line react-hooks/exhaustive-deps
  const approvals = useStore((s) => s.approvals);
  const waiting = useMemo(() => new Set(Object.values(approvals).filter((a) => !a.resolved).map((a) => a.session)), [approvals]);
  const current = useNav((n) => uiOf(n, dev).session);

  const pick = (id: string) => { setUi(dev, { session: id }); loadSession(id); };

  return (
    <aside className="flex min-h-0 flex-col gap-2 border-r border-line-soft bg-chrome p-2" aria-label="Chats">
      <button className="btn sm justify-center" onClick={() => setUi(dev, { session: null })}><Icon name="plus" />New chat</button>
      {list.length === 0
        ? <p className="px-2 pt-2 text-xs text-text-3">No chats with {label(dev)} yet</p>
        : <div className="flex min-h-0 flex-col gap-0.5 overflow-auto">
            {list.map((x) => (
              <button key={x.id} title={`Updated ${ago(x.updated_ms)}`} aria-current={x.id === current || undefined} onClick={() => pick(x.id)}
                className={`flex h-7 flex-none items-center gap-2 rounded-md px-2.5 text-left transition-colors ${x.id === current ? "bg-surface-2 text-text" : "text-text-2 hover:bg-surface hover:text-text"}`}>
                {x.status === "running" && !waiting.has(x.id) && <><span className="dot pulse" aria-hidden /><span className="sr-only">Running</span></>}
                <span className="min-w-0 flex-1 truncate">{x.title || "Untitled chat"}</span>
                {waiting.has(x.id) && <span className="tag custom">waiting</span>}
                {x.status === "failed" && <span className="tag off">failed</span>}
              </button>
            ))}
          </div>}
    </aside>
  );
}
