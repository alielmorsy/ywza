import { useEffect, useRef, type KeyboardEvent } from "react";
import { Icon } from "../../Icon";
import { approve, type Approval } from "../../api";

/** `newest`: the latest pending card; it takes focus on mount unless the user is typing. A hidden chat can't take focus. */
export function ApprovalCard({ a, newest }: { a: Approval; newest: boolean }) {
  const allow = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const el = document.activeElement;
    const typing = el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement;
    if (newest && !a.resolved && !typing) allow.current?.focus();
  }, [newest, a.resolved]);

  if (a.resolved) {
    return (
      <div className="flex min-w-0 items-center gap-2 text-xs text-text-2">
        <Icon name={a.allowed ? "check" : "x"} className="size-3.5" />
        <span className="min-w-0 truncate">{a.allowed ? "Allowed" : "Denied"}: <span className="mono">{a.tool}</span> {a.summary}</span>
      </div>
    );
  }

  const keys = (e: KeyboardEvent) => {
    if (e.key === "Escape") { e.preventDefault(); approve(a.request, false); }
    else if (e.key === "Enter" && !(e.target instanceof HTMLButtonElement)) { e.preventDefault(); approve(a.request, true); }
  };

  return (
    <div role="group" aria-label={`Approval needed: ${a.tool}`} tabIndex={-1} onKeyDown={keys}
      className="rounded-md border border-warn/45 bg-warn-soft px-3 py-2.5 outline-none">
      <div className="flex items-center gap-2 font-semibold text-warn"><Icon name="shield" /><span>Approval needed</span><span className="mono min-w-0 truncate">{a.tool}</span></div>
      <div className="mt-1 break-words text-text select-text">{a.summary}</div>
      {a.detail && (
        <div className="mono mt-2 max-h-40 overflow-auto rounded-md bg-chrome px-2 py-1.5 text-xs leading-[1.6] whitespace-pre-wrap break-words text-text-2 select-text">{a.detail}</div>
      )}
      <div className="mt-2.5 flex flex-wrap gap-1.5">
        <button className="btn sm" title="Deny (Esc)" onClick={() => approve(a.request, false)}>Deny</button>
        <button ref={allow} className="btn sm primary" title="Allow (Enter)" onClick={() => approve(a.request, true)}>Allow</button>
        <button className="btn sm" title="Allow now and add a rule so this kind of call is not asked again" onClick={() => approve(a.request, true, true)}>Always allow</button>
      </div>
    </div>
  );
}
