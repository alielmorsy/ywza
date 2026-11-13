import { useEffect, useLayoutEffect, useRef } from "react";
import { loadSession } from "../api";
import { Markdown } from "../views/chat/Markdown";
import { ShownImage } from "../views/chat/ShownImage";
import { ApproveCard } from "./ApproveCard";
import { pendingOf, type Run } from "./runs";
import { StepLine } from "./ui";

/** The sheet's compact transcript of one run. */
export function Feed({ run }: { run?: Run }) {
  const box = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const size = run ? run.steps.length + run.approvals.length + run.turns.length : 0;
  useEffect(() => { if (run) loadSession(run.id, run.driver); }, [run?.id, run?.driver]); // eslint-disable-line react-hooks/exhaustive-deps
  useLayoutEffect(() => { if (stick.current) box.current?.scrollTo(0, box.current.scrollHeight); }, [size, run?.id, run?.state]);

  return (
    <div ref={box} className="m-scroll flex min-h-0 flex-1 flex-col gap-2 px-3 pt-1 pb-2 select-text"
      onScroll={(e) => { const b = e.currentTarget; stick.current = b.scrollHeight - b.scrollTop - b.clientHeight < 80; }}>
      {!run && <p className="py-5 text-center text-text-2">Send a message to start a new run.</p>}
      {run?.turns.map((t, i) => (
        <div key={i} className="flex flex-col gap-2">
          {t.user && <div className="max-w-[84%] self-end rounded-[10px_10px_2px_10px] bg-surface-3 px-3 py-2 break-words whitespace-pre-wrap">{t.user}</div>}
          {t.items.map((x, j) => x.k === "step" ? <StepLine key={x.key} step={x} />
            : x.k === "img" ? <ShownImage key={j} png={x.png} />
            : x.k === "say" ? <Markdown key={j} className="max-w-[88%] rounded-[2px_10px_10px_10px] border border-line-soft bg-surface px-3 py-2">{x.text}</Markdown>
            : <p key={j} className="max-w-[88%] rounded-[2px_10px_10px_10px] border border-bad px-3 py-2 break-words text-bad">{x.text}</p>)}
        </div>
      ))}
      {run?.approvals.map((a) => <ApproveCard key={a.request} a={a} device={run.current} />)}
      {run && pendingOf(run).length === 0 && run.state === "working" && <p className="animate-pulse px-1 text-text-3" role="status">Working</p>}
    </div>
  );
}
