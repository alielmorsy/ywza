import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Icon } from "../../Icon";
import { approve, type Approval } from "../../api";

/** `newest`: the latest pending card. It takes focus only when nothing else has it or focus is already in this chat,
    so it never pulls the keyboard from a field or from a device being controlled. The status bar announces it. */
export function ApprovalCard({ a, newest }: { a: Approval; newest: boolean }) {
  const group = useRef<HTMLDivElement>(null);
  const [remember, setRemember] = useState(false);

  useEffect(() => {
    const el = document.activeElement;
    const free = !el || el === document.body || (!!el.closest("[data-messages]") && el.tagName !== "TEXTAREA");
    if (newest && !a.resolved && free) group.current?.focus();
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
    if (e.key === "Escape") { e.preventDefault(); if (remember) setRemember(false); else approve(a.request, false); }

  };

  return (
    <div ref={group} role="group" aria-label={`Approval needed: ${a.tool}`} tabIndex={-1} onKeyDown={keys}
      className="rounded-md border border-warn/45 bg-warn-soft px-3 py-2.5">
      <div className="flex items-center gap-2 font-semibold text-warn"><Icon name="shield" /><span>Approval needed</span><span className="mono min-w-0 truncate">{a.tool}</span></div>
      {a.image && <img src={`data:image/png;base64,${a.image}`} alt="The element this action targets, boxed" className="mt-2 max-h-56 max-w-full rounded-md border border-line object-contain" />}
      <div className="mt-1.5 break-words text-text select-text">{a.summary}</div>
      {a.detail && (
        <details className="mt-1.5">
          <summary className="w-fit cursor-pointer text-xs text-text-2 hover:text-text">Show arguments</summary>
          <div className="mono mt-1.5 max-h-40 overflow-auto rounded-md bg-chrome px-2 py-1.5 text-xs leading-[1.6] whitespace-pre-wrap break-words text-text-2 select-text">{a.detail}</div>
        </details>
      )}
      <div className="mt-2.5 flex flex-wrap gap-1.5">
        <button className="btn sm" title="Deny (Esc)" onClick={() => approve(a.request, false)}>Deny</button>
        <button className="btn sm primary" title="Allow this action once" onClick={() => approve(a.request, true)}>Allow once</button>
        <button className="btn sm" title="Allow now and add a rule so this kind of call is not asked again" onClick={() => setRemember(!remember)} aria-expanded={remember}>Always allow...</button>
      </div>
      {remember && <div className="mt-3 border-t border-line pt-3">
        <p className="text-xs text-text-2">Allow all future calls to <strong className="select-text text-text">{a.tool}</strong>, with any arguments, under Follow rules. This changes this PC's approval rules. You can remove the rule in Settings.</p>
        <div className="mt-2 flex gap-2"><button className="btn sm" onClick={() => setRemember(false)}>Cancel</button><button className="btn sm primary" onClick={() => approve(a.request, true, true)}>Allow and save rule</button></div>
      </div>}
    </div>
  );
}
