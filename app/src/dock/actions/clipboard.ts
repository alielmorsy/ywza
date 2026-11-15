import type { ContextItem } from "../../api";

/** Errors, stack traces and failed commands. */
export const looksLikeError = (t: string) =>
  /\b(error|exception|traceback|panicked|fatal|failed|failure|denied|not found|cannot|unable to)\b/i.test(t) ||
  /^\s+at .+[(:]\d+/m.test(t) || /File ".+", line \d+/.test(t);

export function clipboardActions(items: ContextItem[]): { label: string; prompt: string }[] {
  if (items.some((i) => i.kind === "image")) return [
    { label: "Read the text in this image", prompt: "Open the image above and give me all the text in it, as plain text." },
    { label: "Explain this screenshot", prompt: "Open the image above and explain what it shows." },
  ];
  const paths = items.filter((i) => i.kind === "path").length;
  if (paths) return [{ label: paths > 1 ? "Summarize these files" : "Summarize this file", prompt: `Summarize ${paths > 1 ? "these files" : "this file"}.` }];
  const text = items.find((i) => i.kind === "text")?.text;
  if (!text) return [];
  return [
    { label: "Explain", prompt: "Explain the copied text above." },
    ...(looksLikeError(text) ? [{ label: "Fix this error in a terminal", prompt: "The copied text above is an error. Find its cause and fix it, using a terminal." }] : []),
    { label: "Fill the focused form with this", prompt: "Fill the form in the window I was in with the copied text above, matching each value to its field." },
    { label: "Turn into a table in the focused app", prompt: "Turn the copied text above into a table in the window I was in." },
  ];
}
