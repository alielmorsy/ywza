import type { Device } from "../api";
import { isActive, pendingOf, type Run } from "./runs";

export type DevState = "idle" | "working" | "need" | "error" | "offline";
export const DEV_TEXT: Record<DevState, string> = { idle: "Idle", working: "Working", need: "Needs you", error: "Error", offline: "Offline" };
export const DEV_COLOR: Record<DevState, string> = {
  idle: "var(--c-text-2)", working: "var(--c-accent)", need: "var(--c-warn)", error: "var(--c-bad)", offline: "var(--c-faint)",
};
export interface TileInfo { state: DevState; ticker: string; here: boolean; run?: Run }

const FAILED_FOR = 30 * 60_000;
const sentence = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

export function tileInfo(name: string, d: Device | undefined, runs: Run[]): TileInfo {
  if (!d?.connected) return { state: "offline", ticker: d?.detail ? sentence(d.detail) : "Offline", here: false };
  const active = runs.filter(isActive);
  const acting = active.find((r) => r.current === name);
  const run = acting ?? active.find((r) => r.driver === name);
  if (acting?.state === "needs") return { state: "need", ticker: `Waiting for you: ${pendingOf(acting)[0]?.tool ?? acting.help?.reason ?? ""}`, here: true, run: acting };
  if (run) return { state: "working", ticker: acting ? acting.last || "Working" : `Driving: ${run.title}`, here: !!acting, run };
  const failed = runs.find((r) => r.state === "failed" && (r.current === name || r.driver === name) && Date.now() - r.updated < FAILED_FOR);
  if (failed) return { state: "error", ticker: failed.last || "Run failed", here: false, run: failed };
  return { state: "idle", ticker: `Idle${d.os ? ` · ${d.os.split(" ").slice(0, 2).join(" ")}` : ""}`, here: false };
}
