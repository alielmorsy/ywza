import { useEffect } from "react";
import { Icon } from "../Icon";
import { LOCAL, send, useStore, type ApprovalAction, type ApprovalRule } from "../api";

interface Props { rules: ApprovalRule[]; up: (fn: (rules: ApprovalRule[]) => void) => void }

export function ApprovalRules({ rules, up }: Props) {
  const tools = useStore((s) => s.tools);
  const connected = useStore((s) => !!s.devices[LOCAL]?.connected);
  useEffect(() => { if (connected) send(LOCAL, { type: "list_tools" }); }, [connected]);
  const names = tools.map((t) => t.name).sort();
  return (
    <>
      {rules.map((r, i) => (
        <div key={i} className="flex min-h-[44px] items-center gap-1.5 border-t border-line-soft px-3.5 py-2 first:border-t-0">
          <select className="field mono w-[170px] flex-none" aria-label="Tool" value={r.tool}
            title={tools.find((t) => t.name === r.tool)?.description}
            onChange={(e) => up((l) => (l[i].tool = e.target.value))}>
            {!r.tool && <option value="" disabled>Pick a tool</option>}
            {r.tool && !names.includes(r.tool) && <option value={r.tool}>{r.tool}</option>}
            {names.map((n) => <option key={n} value={n}>{n}</option>)}
          </select>
          <input className="field mono min-w-0 flex-1" aria-label="Pattern" placeholder="any" value={r.pattern ?? ""} spellCheck={false}
            onChange={(e) => up((l) => (l[i].pattern = e.target.value || null))} />
          <select className="field w-[90px] flex-none" aria-label="Action" value={r.action}
            onChange={(e) => up((l) => (l[i].action = e.target.value as ApprovalAction))}>
            <option value="ask">Ask</option>
            <option value="allow">Allow</option>
            <option value="deny">Deny</option>
          </select>
          <button type="button" className="btn ghost sm flex-none" title="Remove rule" aria-label="Remove rule" onClick={() => up((l) => { l.splice(i, 1); })}>
            <Icon name="trash" />
          </button>
        </div>
      ))}
      <div className="flex min-h-[44px] items-center gap-3 border-t border-line-soft px-3.5 py-2 first:border-t-0">
        <button type="button" className="btn sm flex-none" onClick={() => up((l) => { l.push({ tool: "", pattern: null, action: "ask" }); })}>
          <Icon name="plus" />Add rule
        </button>
        <span className="min-w-0 text-xs text-text-2">First matching rule wins. Unmatched tools run without asking.</span>
      </div>
    </>
  );
}
