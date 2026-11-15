import { LOCAL, prompt, type Effort } from "../api";
import { dockApproval } from "./approval";

/** Sends `text` to this PC's agent with the main chat's model and effort and the dock's approval mode. */
export function ask(text: string, session: string | null) {
  let c: { model?: string; effort?: Effort } = {};
  try { c = JSON.parse(localStorage.getItem("ywza.composer") ?? "{}"); } catch { /* private mode */ }
  return prompt(LOCAL, text, session, { model: c.model, effort: c.effort, approval: dockApproval() });
}
