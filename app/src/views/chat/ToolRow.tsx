import { Icon } from "../../Icon";
import type { SEvent } from "../../api";

type Call = Extract<SEvent, { type: "tool_call" }>;
type Result = Extract<SEvent, { type: "tool_result" }>;

const short = (v: unknown) => {
  const s = typeof v === "string" ? v : JSON.stringify(v) ?? "";
  return s.length > 90 ? s.slice(0, 90) + "..." : s;
};

/** `waiting`: this is the last call and the session is still running, so it is in flight. `asking`: it waits for an approval. */
export function ToolRow({ call, res, waiting, asking }: { call: Call; res?: Result; waiting: boolean; asking?: boolean }) {
  const failed = !!res && !res.ok;
  return (
    <details className="group overflow-hidden rounded-md border border-line-soft bg-surface">
      <summary className="flex cursor-pointer list-none items-center gap-2 px-2.5 py-1.5 text-xs text-text-2 hover:bg-surface-2 [&::-webkit-details-marker]:hidden">
        {res ? (failed ? <Icon name="warn" className="text-bad" /> : <Icon name="check" />)
          : waiting && asking ? <span className="dot warn mx-0.5" role="img" aria-label="Waiting for approval" />
          : waiting ? <span className="size-3 flex-none animate-spin rounded-full border-2 border-surface-3 border-t-accent" role="status" aria-label="Running" />
          : <span className="dot idle" role="img" aria-label="No result" />}
        <span className={`mono ${failed ? "text-bad" : "text-text"}`}>{call.name}</span>
        <span className="mono min-w-0 flex-1 truncate text-text-2">{short(call.args)}</span>
        {!res && waiting && asking && <span className="flex-none text-warn">Waiting for approval</span>}
        <Icon name="chev" className="transition-transform duration-200 ease-out-expo group-open:rotate-180" />
      </summary>
      <div className="mono max-h-60 overflow-auto border-t border-line-soft px-2.5 py-2 text-xs leading-[1.6] whitespace-pre-wrap break-words text-text-2 select-text">
        <div className="text-text-2">{JSON.stringify(call.args, null, 2)}</div>
        {res && <div className={`mt-1.5 ${failed ? "text-bad" : ""}`}>{res.text}{res.has_image ? "\n(image attached)" : ""}</div>}
        {!res && !waiting && <div className="mt-1.5 text-text-3">No result. The chat ended before this call finished.</div>}
      </div>
    </details>
  );
}
