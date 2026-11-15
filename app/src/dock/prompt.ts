import { LOCAL, prompt, type ApprovalMode, type Effort } from "../api";

/** Sends `text` to this PC's agent with the main chat's model, effort and approval mode. */
export function ask(text: string, session: string | null) {
  let c: { model?: string; effort?: Effort; approval?: ApprovalMode } = {};
  try { c = JSON.parse(localStorage.getItem("ywza.composer") ?? "{}"); } catch { /* private mode */ }
  return prompt(LOCAL, text, session, { model: c.model, effort: c.effort, approval: c.approval ?? "rules" });
}
