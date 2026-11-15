// Dock state machine. `approval` is an overlay over any mode: resolving it keeps the mode, so a dock
// that opened only for an approval goes back to hidden.
export type Mode = "hidden" | "prompt" | "working" | "done" | "failed" | "expanded";
export type DockState = { mode: Mode; session?: string; approval?: number };
export type DockEvent =
  | { t: "hotkey" }
  | { t: "submit"; session: string }
  | { t: "esc"; running?: boolean }
  | { t: "blur"; running: boolean }
  | { t: "approval"; request: number }
  | { t: "resolved"; request: number }
  | { t: "done" }
  | { t: "failed" }
  | { t: "click_tab" }
  | { t: "timeout" }
  | { t: "cancel" };

// Built key by key: no undefined fields, so states compare equal.
const to = (s: DockState, mode: Mode, session = s.session): DockState => ({
  mode,
  ...(session !== undefined ? { session } : {}),
  ...(s.approval !== undefined ? { approval: s.approval } : {}),
});

export function step(s: DockState, e: DockEvent): DockState {
  const { mode } = s;
  switch (e.t) {
    case "hotkey":
      if (mode === "hidden") return to(s, "prompt");
      if (mode === "prompt" || mode === "expanded") return to(s, "hidden");
      return to(s, "expanded");
    case "submit":
      return mode === "prompt" ? to(s, "working", e.session) : s;
    case "esc":
      if (mode === "expanded") return to(s, e.running ? "working" : "hidden");
      return mode === "prompt" || mode === "done" || mode === "failed" ? to(s, "hidden") : s;
    case "blur":
      if (mode === "expanded") return to(s, e.running ? "working" : "hidden");
      return mode === "prompt" ? to(s, "hidden") : s;
    case "approval":
      return { ...s, approval: e.request };
    case "resolved": {
      if (s.approval !== e.request) return s;
      const { approval: _, ...rest } = s;
      return rest;
    }
    case "done":
      return mode === "working" ? to(s, "done") : s;
    case "failed":
      return mode === "working" ? to(s, "failed") : s;
    case "click_tab":
      return mode === "working" || mode === "done" || mode === "failed" ? to(s, "expanded") : s;
    case "cancel":
      return mode === "working" ? to(s, "hidden") : s;
    case "timeout":
      return mode === "done" ? to(s, "hidden") : s;
  }
}
