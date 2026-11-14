import { useLayoutEffect, useRef } from "react";
import { Icon } from "../../Icon";
import { helpOf, type Approval, type SEvent } from "../../api";
import { setPairing } from "../../nav";
import { LetIn } from "../PairDialog";
import { ApprovalCard } from "./ApprovalCard";
import { HelpCard } from "./HelpCard";
import { Markdown } from "./Markdown";
import { Receipt } from "./Receipt";
import { ResumedRow } from "./ResumedRow";
import { ShownImage } from "./ShownImage";
import { ToolRow } from "./ToolRow";

type Result = Extract<SEvent, { type: "tool_result" }>;

const EXAMPLES = ["What's open on the screen?", "Open Notepad and write a shopping list"];

/** `alone`: This PC with nothing paired yet; the empty chat leads with pairing. */
interface Props { who: string; dev: string; session: string | null; events: SEvent[]; running: boolean; approvals: Approval[]; fresh: boolean; alone?: boolean; compact?: boolean; onExample: (t: string) => void }

export function Messages({ who, dev, session, events, running, approvals, fresh, alone, compact, onExample }: Props) {
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
  const help = helpOf(events);
  const col = compact ? "" : "mx-auto w-full max-w-[720px]";
  const prose = `${col} leading-[1.5] break-words text-text select-text`;

  return (
    <div ref={box} data-messages onScroll={(e) => { const b = e.currentTarget; stick.current = b.scrollHeight - b.scrollTop - b.clientHeight < 80; }}
      className={`flex min-h-0 flex-1 flex-col overflow-auto ${compact ? "gap-2.5 px-2.5 pt-2.5 pb-1.5" : "gap-3.5 px-6 pt-5 pb-3"}`}>
      {fresh && events.length === 0 && (
        <div className="m-auto flex max-w-[420px] flex-col items-center gap-3 text-center">
          {alone && <>
            <h2 className="text-base leading-6">Pair your other devices</h2>
            <p className="text-text-2">Reach your other computers and your phone from here, with no open ports. Pair one with a one-time code, then chat with it or watch its screen.</p>
            <div className="w-full rounded-md border border-line bg-surface text-left"><LetIn /></div>
            <button className="btn ghost sm" onClick={() => setPairing(true)}>The other device shows a code? Connect to it</button>
            <span className="mt-5 h-px w-16 bg-line" aria-hidden />
          </>}
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
            return <div key={i} className={col}><ToolRow call={e} res={results.get(e.id)} waiting={running && i === lastCall} asking={!!newest} /></div>;
          case "tool_result":
            return e.image ? <div key={i} className={col}><ShownImage png={e.image} /></div> : null;
          case "done":
            return e.summary.trim() && e.summary.trim() !== lastAssistant ? <Markdown key={i} className={prose}>{e.summary}</Markdown> : null;
          case "usage":
            return <Receipt key={i} u={e} className={col} />;
          case "failed":
            return <div key={i} className={`${col} flex items-start gap-2 break-words whitespace-pre-wrap text-bad select-text`}><Icon name="warn" className="mt-px" />{e.error}</div>;
          case "help":
            return session ? <div key={i} className={col}><HelpCard h={e} open={e === help} dev={dev} session={session} /></div> : null;
          case "resumed":
            return <div key={i} className={col}><ResumedRow digest={e.digest} /></div>;
          default: return null;
        }
      })}
      {approvals.map((a) => <div key={a.request} className={col}><ApprovalCard a={a} newest={a === newest} /></div>)}
      {running && !newest && !help && <div className={`${col} animate-pulse text-text-3`} role="status">Working</div>}
    </div>
  );
}
