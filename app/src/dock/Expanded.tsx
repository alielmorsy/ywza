import { useEffect, useState } from "react";
import { Icon } from "../Icon";
import { LOCAL, cancel, loadSession, useStore, type SEvent } from "../api";
import { Messages } from "../views/chat/Messages";
import { Tab, type TabKind } from "./Tab";
import { ApprovalSelect } from "./approval";
import { ask } from "./prompt";

const NONE: SEvent[] = [];
const noop = () => {};

/** Transcript of the dock's session with a one-line follow-up box. */
export function Expanded({ session, onClose }: { session: string; onClose: () => void }) {
  const events = useStore((s) => s.events[session]) ?? NONE;
  const status = useStore((s) => s.sessions[session]?.status);
  const title = useStore((s) => s.sessions[session]?.title ?? "");
  const [text, setText] = useState("");
  const [sent, setSent] = useState(false);
  const running = status === "running";
  const kind: TabKind = running ? "working" : status === "failed" ? "failed" : "done";

  useEffect(() => { loadSession(session); }, [session]);
  useEffect(() => { setSent(false); }, [events.length, running]);

  const send = () => {
    const t = text.trim();
    if (!t || running || sent) return;
    ask(t, session);
    setSent(true);
    setText("");
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <Tab kind={kind} line={title} onStop={running ? () => cancel(session) : undefined} onClose={onClose} />
      <Messages who="This PC" dev={LOCAL} session={session} events={events} running={running} approvals={[]} fresh={false} compact onExample={noop} />
      <div className="flex flex-none items-center gap-1.5 border-t border-line-soft p-2">
        <input value={text} aria-label="Follow-up" spellCheck={false} autoComplete="off" disabled={running || sent}
          placeholder={running ? "Stop the run to send a follow-up" : "Follow up"} className="field flex-1"
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter" && !e.nativeEvent.isComposing) { e.preventDefault(); send(); } }} />
        <ApprovalSelect />
        <button type="button" className="btn primary size-[30px] flex-none justify-center p-0 aria-disabled:opacity-40" title="Send (Enter)" aria-label="Send"
          aria-disabled={running || sent || !text.trim()} onClick={send}><Icon name="send" /></button>
      </div>
    </div>
  );
}
