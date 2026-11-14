import { useState } from "react";
import { approve, useStore, type Approval } from "../api";
import { Icon } from "../Icon";
import { DevChip } from "./ui";

/** A tool call waiting on you. `device`: where it runs; `run`: shown when the card is not already inside that run. */
export function ApproveCard({ a, device, run }: { a: Approval; device?: string; run?: string }) {
  const [remember, setRemember] = useState(false);
  const [sent, setSent] = useState(false); // until the agent answers with approval_resolved
  // The agent ignores "remember" from a client without the settings permission.
  const canRemember = useStore((s) => !a.device || s.devices[a.device]?.permissions?.settings !== false);
  const go = (allow: boolean, always = false) => { setSent(true); setTimeout(() => setSent(false), 8000); approve(a.request, allow, always, a.device); };

  if (a.resolved) {
    return (
      <div className="flex items-center gap-2 px-1 py-1 text-xs text-text-2">
        <Icon name={a.allowed ? "check" : "x"} className="!size-3.5" />
        <span className="min-w-0 truncate">{a.allowed ? "Allowed" : "Denied"}: <span className="mono">{a.tool}</span> {a.summary}</span>
      </div>
    );
  }
  return (
    <div role="group" aria-label={`Approval needed: ${a.tool}`} className="flex flex-col gap-2 rounded-xl border border-warn/50 bg-warn-soft p-3">
      <div className="flex min-w-0 items-center gap-2 text-[13px]">
        <Icon name="shield" className="text-warn" />
        <b className="font-semibold text-warn">Approval needed</b>
        {device && <DevChip name={device} />}
        {run && <span className="min-w-0 flex-1 truncate text-right text-xs text-text-2">{run}</span>}
      </div>
      {a.image && <img src={`data:image/png;base64,${a.image}`} alt="The element this action targets, boxed" className="max-h-48 max-w-full self-start rounded-md border border-line object-contain" />}
      <div className="break-words select-text"><code className="mono text-warn">{a.tool}</code> <span className="text-text">{a.summary}</span></div>
      {a.detail && (
        <details>
          <summary className="inline-flex min-h-8 cursor-pointer items-center text-xs text-text-2">Show arguments</summary>
          <pre className="mono max-h-40 overflow-auto rounded-md bg-chrome p-2 text-[11.5px] leading-relaxed whitespace-pre-wrap break-words text-text-2 select-text">{a.detail}</pre>
        </details>
      )}
      <div className="flex flex-wrap gap-2">
        <button className="m-btn primary sm" disabled={sent} onClick={() => go(true)}>Allow</button>
        <button className="m-btn sm" disabled={sent} onClick={() => go(false)}>Deny</button>
        {canRemember && <button className="m-btn ghost sm" disabled={sent} aria-expanded={remember} onClick={() => setRemember(!remember)}>Always allow</button>}
      </div>
      {remember && (
        <div className="flex flex-col gap-2 border-t border-line pt-2.5">
          <p className="text-xs text-text-2">Allow every future call to <b className="text-text">{a.tool}</b> with any arguments. This changes {a.device ?? "this device"}'s approval rules; remove the rule in its Settings.</p>
          <div className="flex gap-2">
            <button className="m-btn sm" onClick={() => setRemember(false)}>Cancel</button>
            <button className="m-btn primary sm" disabled={sent} onClick={() => go(true, true)}>Allow and remember</button>
          </div>
        </div>
      )}
    </div>
  );
}
