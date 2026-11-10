import { LOCAL, send, type ApprovalRule, type Config } from "../api";
import { flash } from "../nav";

/** Config as the editors see it. The agent masks api_key, so it is blanked: empty means "keep the saved key". */
export const cleanConfig = (c: Config): Config => ({
  ...structuredClone(c),
  relay_url: c.relay_url ?? null,
  approval: { rules: (c.approval?.rules ?? []).map((r) => ({ ...r, pattern: r.pattern ?? null })) },
  llm: { ...c.llm, api_key: "", effort: c.llm.effort ?? null },
});

/** Drops rows without a tool and empty patterns. */
export const trimRules = (rules: ApprovalRule[]): ApprovalRule[] =>
  rules.filter((r) => r.tool.trim()).map((r) => ({ tool: r.tool.trim(), ...(r.pattern?.trim() && { pattern: r.pattern.trim() }), action: r.action }));

let confirmation: string | null = null;

/** Sends the config; the confirmation shows once the agent answers with the stored config (App calls takeSaved). */
export function saveConfig(config: Config, message: string) {
  confirmation = message;
  send(LOCAL, { type: "set_config", config });
  send(LOCAL, { type: "get_config" });
  flash("Saving...");
}

export const takeSaved = () => { const m = confirmation; confirmation = null; return m; };
