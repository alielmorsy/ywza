import { LOCAL, type SEvent } from "../../api";

export const label = (dev: string) => (dev === LOCAL ? "This PC" : dev);

export function ago(ms: number) {
  const s = Math.max(0, (Date.now() - ms) / 1000);
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  return `${Math.floor(s / 86400)} d ago`;
}

/** The window the AI last acted on: newest tool_call naming one in `args.window` or `args.target.window`. */
export function agentWindow(events: SEvent[] | undefined): string | null {
  for (let i = (events?.length ?? 0) - 1; i >= 0; i--) {
    const e = events![i];
    if (e.type !== "tool_call") continue;
    const w = e.args?.window ?? e.args?.target?.window;
    if (typeof w === "string") return w;
  }
  return null;
}
