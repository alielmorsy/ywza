import { useEffect, useMemo, useRef, useState } from "react";
import { LOCAL, getState, loadSession, sessionsOf, useStore, type Approval, type SEvent } from "../../api";
import { setUi, uiOf, useNav } from "../../nav";
import { Composer } from "./Composer";
import { Messages } from "./Messages";
import { label } from "./util";

const NONE: SEvent[] = [];
const NO_APPROVALS: Approval[] = [];

/** Reads its session from nav so the chat view, the split view and the live dock always show the same chat. */
export function ChatPane({ dev, compact }: { dev: string; compact?: boolean }) {
  const session = useNav((n) => uiOf(n, dev).session);
  const events = useStore((s) => (session && s.events[session]) || NONE);
  const running = useStore((s) => !!session && s.sessions[session]?.status === "running");
  const all = useStore((s) => s.approvals);
  const approvals = useMemo(
    () => (session ? Object.values(all).filter((a) => a.session === session).sort((a, b) => a.request - b.request) : NO_APPROVALS),
    [all, session],
  );
  const alone = useStore((s) => dev === LOCAL && Object.keys(s.devices).length <= 1);
  const [text, setText] = useState("");
  const input = useRef<HTMLTextAreaElement>(null);

  useEffect(() => { if (session && getState().sessions[session]) loadSession(session); }, [session]);

  const fill = (t: string) => { setText(t); input.current?.focus(); };

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      {compact && <Switcher dev={dev} session={session} />}
      <Messages key={session ?? "new"} who={label(dev)} events={events} running={running} approvals={approvals} fresh={!session} alone={alone && !compact} compact={compact} onExample={fill} />
      <Composer dev={dev} session={session} running={running} compact={compact} text={text} setText={setText} inputRef={input} />
    </div>
  );
}

function Switcher({ dev, session }: { dev: string; session: string | null }) {
  const sessions = useStore((s) => s.sessions);
  const list = useMemo(() => sessionsOf(getState(), dev), [sessions, dev]); // eslint-disable-line react-hooks/exhaustive-deps
  const known = !session || list.some((x) => x.id === session);
  return (
    <div className="flex-none border-b border-line-soft px-2.5 py-1.5">
      <select className="field h-[26px] text-xs" aria-label="Chat" value={session ?? ""}
        onChange={(e) => { const id = e.target.value || null; setUi(dev, { session: id }); if (id) loadSession(id); }}>
        <option value="">New chat</option>
        {!known && <option value={session}>New chat</option>}
        {list.map((x) => <option key={x.id} value={x.id}>{x.title || "Untitled chat"}</option>)}
      </select>
    </div>
  );
}
