// Runs as the phone shows them: sessions of the paired devices' own agents, with their events folded into steps.
import { getState, useStore, type Approval, type Sid, type State } from "../api";
import { phoneName } from "./devices";

export type RunState = "working" | "needs" | "done" | "failed" | "offline";
export interface Step { k: "step"; key: string; device: string; text: string; bad: boolean; live: boolean }
export type Item = Step | { k: "say"; text: string } | { k: "err"; text: string };
/** One user message and what the driver did about it: says before the first step are the plan, steps the acts, later says the check. */
export interface Turn { user: string; items: Item[] }
export interface Run {
  id: Sid; title: string; driver: string; scope: string[]; state: RunState; current: string;
  turns: Turn[]; steps: Step[]; approvals: Approval[]; updated: number; since?: number; last: string;
}

const short = (v: unknown) => {
  const s = typeof v === "string" ? v : JSON.stringify(v) ?? "";
  return s === "{}" ? "" : s.length > 60 ? s.slice(0, 60) + "..." : s;
};

/** When this app first saw each session running; the protocol carries no start time. */
const since = new Map<Sid, number>();

function build(s: State): Run[] {
  const apps = Object.values(s.remoteApprovals);
  return Object.values(s.remote).map((x) => {
    const driver = x.driver ?? "";
    const turns: Turn[] = [];
    const steps = new Map<string, Step>();
    const answered = new Set<string>();
    let cur: Turn | undefined;
    const turn = () => cur ?? (cur = turns[turns.push({ user: "", items: [] }) - 1]);
    let lastSay = "";
    for (const e of s.events[x.id] ?? []) {
      if (e.type === "user") cur = turns[turns.push({ user: e.text, items: [] }) - 1];
      else if (e.type === "assistant") { if (e.text.trim()) { lastSay = e.text.trim(); turn().items.push({ k: "say", text: e.text }); } }
      else if (e.type === "tool_call") {
        const st: Step = { k: "step", key: e.id, device: phoneName(s, driver, e.device), text: `${e.name} ${short(e.args)}`.trim(), bad: false, live: false };
        steps.set(e.id, st);
        turn().items.push(st);
      } else if (e.type === "tool_result") {
        answered.add(e.id);
        const st = steps.get(e.id);
        if (st) st.bad = !e.ok;
      } else if (e.type === "done") { if (e.summary.trim() && e.summary.trim() !== lastSay) turn().items.push({ k: "say", text: e.summary }); }
      else if (e.type === "failed") turn().items.push({ k: "err", text: e.error });
    }
    const all = [...steps.values()];
    const end = all[all.length - 1];
    if (end && x.status === "running" && !answered.has(end.key)) end.live = true;
    if (x.status === "running" && !since.has(x.id)) since.set(x.id, Date.now());
    const mine = apps.filter((a) => a.device === driver && a.session === x.id);
    const state: RunState = x.status === "failed" ? "failed" : x.status === "done" ? "done" : !s.devices[driver]?.connected ? "offline" : mine.some((a) => !a.resolved) ? "needs" : "working";
    return {
      id: x.id, title: x.title || "Untitled run", driver, state, turns, steps: all, approvals: mine, updated: x.updated_ms,
      since: x.status === "running" ? since.get(x.id) : undefined,
      scope: [...new Set([driver, ...(x.devices ?? []).map((n) => phoneName(s, driver, n))])],
      current: end?.device ?? driver,
      last: end?.text ?? lastSay.split("\n")[0] ?? "",
    };
  }).sort((a, b) => b.updated - a.updated);
}

/** Every screen reads the runs; fold the events once per store change. */
let cache: { deps: unknown[]; runs: Run[] } | undefined;

export function useRuns(): Run[] {
  const deps = [useStore((s) => s.remote), useStore((s) => s.events), useStore((s) => s.remoteApprovals), useStore((s) => s.devices)];
  if (!cache || cache.deps.some((d, i) => d !== deps[i])) cache = { deps, runs: build(getState()) };
  return cache.runs;
}

export const isActive = (r: Run) => r.state === "working" || r.state === "needs";
export const pendingOf = (r: Run) => r.approvals.filter((a) => !a.resolved);
export const STATE_TEXT: Record<RunState, string> = { working: "Running", needs: "Needs you", done: "Done", failed: "Failed", offline: "Driver offline" };

/** Every device a run touched or may use, driver first. */
export const devicesOf = (r: Run) => [...new Set([...r.scope, ...r.steps.map((x) => x.device)])];

export function ago(ms: number) {
  const s = Math.max(0, (Date.now() - ms) / 1000);
  return s < 60 ? "just now" : s < 3600 ? `${Math.floor(s / 60)}m ago` : s < 86400 ? `${Math.floor(s / 3600)}h ago` : `${Math.floor(s / 86400)}d ago`;
}
export const elapsed = (r: Run) => (r.since ? `${Math.max(1, Math.floor((Date.now() - r.since) / 60000))} min` : ago(r.updated));
