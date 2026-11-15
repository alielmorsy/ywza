import { useSyncExternalStore } from "react";
import type { ApprovalMode } from "../api";
import { APPROVALS, approvalWarn } from "../approval";

const KEY = "ywza.dock.approval";
const valid = (v: unknown): v is ApprovalMode => APPROVALS.some((a) => a.value === v);

const read = (): ApprovalMode => {
  try {
    const own = localStorage.getItem(KEY);
    if (valid(own)) return own;
    const c = JSON.parse(localStorage.getItem("ywza.composer") ?? "{}").approval;
    if (valid(c)) return c;
  } catch { /* private mode */ }
  return "rules";
};

let mode = read();
const subs = new Set<() => void>();
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };

export const dockApproval = () => mode;
export const useDockApproval = () => useSyncExternalStore(subscribe, dockApproval);

function setDockApproval(m: ApprovalMode) {
  mode = m;
  try { localStorage.setItem(KEY, m); } catch { /* private mode */ }
  subs.forEach((f) => f());
}

export function ApprovalSelect() {
  const value = useDockApproval();
  return (
    <select aria-label="Approvals" title="Approvals" value={value} onChange={(e) => setDockApproval(e.target.value as ApprovalMode)}
      className={`field h-[26px] w-auto max-w-[150px] min-w-0 flex-none truncate text-xs ${approvalWarn(value)}`}>
      {APPROVALS.map((a) => <option key={a.value} value={a.value}>{a.label}</option>)}
    </select>
  );
}
