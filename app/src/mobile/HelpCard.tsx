import { useState } from "react";
import { resume } from "../api";
import { Icon } from "../Icon";
import { HELP_TITLE } from "../views/chat/HelpCard";
import type { Run } from "./runs";
import { openLive, useLabel } from "./store";

/** The run's agent paused on a blocker: take over in the live view, then resume. `title`: shown when the card is not already inside that run. */
export function HelpCard({ run, title }: { run: Run; title?: string }) {
  const L = useLabel();
  const h = run.help;
  const [note, setNote] = useState("");
  const [sent, setSent] = useState(false); // until the agent answers with `resumed`
  if (!h) return null;
  const go = () => { setSent(true); setTimeout(() => setSent(false), 8000); resume(run.driver, run.id, note); };

  return (
    <div role="group" aria-label={`Needs you: ${HELP_TITLE[h.kind]}`} className="flex flex-col gap-2 rounded-xl border border-warn/50 bg-warn-soft p-3">
      <div className="flex min-w-0 items-center gap-2 text-[13px]">
        <Icon name="shield" className="text-warn" />
        <b className="font-semibold text-warn">Needs you: {HELP_TITLE[h.kind]}</b>
        {title && <span className="min-w-0 flex-1 truncate text-right text-xs text-text-2">{title}</span>}
      </div>
      <div className="break-words text-text select-text">{h.reason}</div>
      {h.title && <div className="truncate text-xs text-text-2">Window: {h.title} on {L(run.current)}</div>}
      <div className="flex gap-2">
        {/* shortcut: opens the whole screen, not zoomed onto h.rect; add a zoomTo rect to LiveState if that is worth it */}
        <button className="m-btn sm" onClick={() => openLive(run.current, { follow: run.id })}>Take over</button>
      </div>
      <div className="flex gap-2">
        <input value={note} onChange={(e) => setNote(e.target.value)} enterKeyHint="send" autoComplete="off" aria-label="Note for the agent" placeholder="Optional note"
          className="h-11 min-w-0 flex-1 rounded-[10px] border border-line bg-chrome px-3 outline-none placeholder:text-text-3 focus:border-accent focus:shadow-[0_0_0_3px_var(--c-accent-soft)]" />
        <button className="m-btn primary sm" disabled={sent} onClick={go}>Resume</button>
      </div>
    </div>
  );
}
