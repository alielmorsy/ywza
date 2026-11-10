import { useLayoutEffect, useRef } from "react";
import { Icon } from "../../Icon";
import type { Approval, SEvent } from "../../api";
import { ApprovalCard } from "./ApprovalCard";
import { Markdown } from "./Markdown";
import { ToolRow } from "./ToolRow";

type Result = Extract<SEvent, { type: "tool_result" }>;

const EXAMPLES = ["What's open on the screen?", "Clean up old files in Downloads"];

interface Props { who: string; events: SEvent[]; running: boolean; approvals: Approval[]; fresh: boolean; compact?: boolean; onExample: (t: string) => void }

export function Messages({ who, events, running, approvals, fresh, compact, onExample }: Props) {
  const box = useRef<HTMLDivElement>(null);
  const stick = useRef(true);

  useLayoutEffect(() => {
    if (stick.current) box.current?.scrollTo(0, box.current.scrollHeight);
  }, [events.length, approvals.length, running]);

  const results = new Map<string, Result>();
  let lastAssistant = "";
  let lastCall = -1;
  events.forEach((e, i) => { if (e.type === "tool_call") lastCall = i; });
  for (const e of events) {
    if (e.type === "tool_result") results.set(e.id, e);
    else if (e.type === "assistant") lastAssistant = e.text.trim();
  }
  const pending = approvals.filter((a) => !a.resolved);
  const newest = pending[pending.length - 1];
  const col = compact ? "" : "mx-auto w-full max-w-[720px]";
  const prose = `${col} leading-[1.5] break-words text-text select-text`;

  return (
    <div ref={box} onScroll={(e) => { const b = e.currentTarget; stick.current = b.scrollHeight - b.scrollTop - b.clientHeight < 80; }}
      className={`flex min-h-0 flex-1 flex-col overflow-auto ${compact ? "gap-2.5 px-2.5 pt-2.5 pb-1.5" : "gap-3.5 px-6 pt-5 pb-3"}`}>
      {fresh && events.length === 0 && (
        <div className="m-auto flex max-w-[420px] flex-col items-center gap-3 text-center">
          <p className="text-text-2">Ask {who} to do something, or start with one of these.</p>
          <div className="flex flex-wrap justify-center gap-1.5">
            {EXAMPLES.map((t) => <button key={t} className="btn sm" onClick={() => onExample(t)}>{t}</button>)}
          </div>
        </div>
      )}
      {events.map((e, i) => {
        switch (e.type) {
          case "user":
            return <div key={i} className={`${col} flex justify-end`}><div className="max-w-[85%] rounded-md rounded-br-sm bg-surface-3 px-3 py-[7px] leading-[1.45] break-words whitespace-pre-wrap select-text">{e.text}</div></div>;
          case "assistant":
            return <Markdown key={i} className={prose}>{e.text}</Markdown>;
          case "tool_call":
            return <div key={i} className={col}><ToolRow call={e} res={results.get(e.id)} waiting={running && i === lastCall} /></div>;
          case "done":
            return e.summary.trim() && e.summary.trim() !== lastAssistant ? <Markdown key={i} className={prose}>{e.summary}</Markdown> : null;
          case "failed":
            return <div key={i} className={`${col} flex items-start gap-2 break-words whitespace-pre-wrap text-bad select-text`}><Icon name="warn" className="mt-px" />{e.error}</div>;
          default: return null;
        }
      })}
      {approvals.map((a) => <div key={a.request} className={col}><ApprovalCard a={a} newest={a === newest} /></div>)}
      {running && !newest && <div className={`${col} animate-pulse text-text-3`} role="status">Working</div>}
    </div>
  );
}
