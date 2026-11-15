import { Icon } from "../Icon";

export type TabKind = "working" | "waiting" | "done" | "failed";
const TITLE: Record<TabKind, string> = { working: "Working", waiting: "Needs you", done: "Done", failed: "Failed" };

interface Props { kind: TabKind; line: string; onOpen?: () => void; onStop?: () => void; onClose?: () => void }

/** The strip that hangs from the top edge. Also the header of the expanded panel. */
export function Tab({ kind, line, onOpen, onStop, onClose }: Props) {
  const live = kind === "working" || kind === "waiting";
  return (
    <div className="flex h-10 flex-none items-center">
      <button type="button" disabled={!onOpen} onClick={onOpen} aria-label="Show the conversation"
        className="flex h-full min-w-0 flex-1 items-center gap-2.5 border-0 bg-transparent pl-4 text-left disabled:cursor-default">
        {live
          ? <span className="dock-dots" data-paused={kind === "waiting" || undefined} aria-hidden><i /><i /><i /></span>
          : <Icon name={kind === "done" ? "check" : "warn"} className={kind === "failed" ? "text-bad" : "text-ok"} />}
        <span className="flex-none font-semibold">{TITLE[kind]}</span>
        <span className={`min-w-0 flex-1 truncate ${kind === "failed" ? "text-bad" : "text-text-2"}`} role="status">{line}</span>
      </button>
      {onStop && <button type="button" className="btn ghost sm mr-1.5 !px-1.5" title="Stop the agent" aria-label="Stop the agent" onClick={onStop}><Icon name="stop" /></button>}
      {onClose && <button type="button" className="btn ghost sm mr-1.5 !px-1.5" title="Close (Esc)" aria-label="Close" onClick={onClose}><Icon name="x" /></button>}
    </div>
  );
}
