import type { ContextItem } from "../../api";

/** Quick actions for files and folders picked in Explorer's right-click menu. */
export function explorerActions(items: ContextItem[]): { label: string; prompt: string }[] {
  const paths = items.filter((i) => i.kind === "path");
  const dirs = paths.filter((p) => p.dir).length;
  const one = paths.length === 1;
  const what = one ? (dirs ? "this folder" : "this file") : dirs ? "these items" : "these files";
  const out = [{ label: `Summarize ${what}`, prompt: `Summarize ${what}.` }];
  if (!one || dirs) out.push({ label: "Organize into folders", prompt: `Propose a folder structure to organize ${what}. Show me the plan and wait for my go-ahead before moving anything.` });
  if (one && dirs) out.push({ label: "Open a terminal here and run the tests", prompt: "Open a terminal in this folder, find out how its tests are run, and run them." });
  if (!dirs) out.push({ label: "Rename consistently", prompt: `Suggest consistent names for ${what}. Show me the plan and wait for my go-ahead before renaming anything.` });
  return out;
}
