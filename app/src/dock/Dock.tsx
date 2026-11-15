// Root of the dock window (index.html?dock). It owns the window's size and position: during a
// transition the window covers the old and new shapes, then shrinks to the new one so transparent
// areas never block the apps below.
import { useEffect, useMemo, useReducer, useRef, useState, type ReactNode } from "react";
import { Sprite } from "../Icon";
import { cancel, dockHide, dockMonitor, dockPlace, dockRestoreFocus, dockShortcut, dockShow, dockTarget, onDockHotkey, useStore, type SEvent } from "../api";
import { ApprovalCard } from "../views/chat/ApprovalCard";
import { Expanded } from "./Expanded";
import { PromptBox } from "./PromptBox";
import { Tab, type TabKind } from "./Tab";
import { CARD_H, bounds, sameRect, shape, union, type Rect, type Shape, type Work } from "./layout";
import { step, type Mode } from "./machine";
import { ask } from "./prompt";
import { DEFAULT_SHORTCUT, savedShortcut } from "./shortcut";
import "./dock.css";

const MS = 240;
const wait = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));
const frames = () => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())));

/** What is on screen: `mode` and `shape` change together, `origin` is the window's rect. */
interface View { origin: Rect; shape: Shape; mode: Mode; scale: number; animating: boolean; leaving: boolean }

const latest = <T extends SEvent["type"]>(events: SEvent[] | undefined, type: T) => {
  for (let i = (events?.length ?? 0) - 1; i >= 0; i--) if (events![i].type === type) return events![i] as Extract<SEvent, { type: T }>;
};
const firstLine = (events: SEvent[] | undefined) => {
  const e = latest(events, "assistant") ?? latest(events, "done");
  return ((e?.type === "assistant" ? e.text : e?.summary) ?? "").trim().split("\n")[0];
};

export default function Dock() {
  const [s, dispatch] = useReducer(step, { mode: "hidden" });
  const [view, setView] = useState<View | null>(null);
  const [cardH, setCardH] = useState(CARD_H);
  const work = useRef<Work | undefined>(undefined);
  const placed = useRef<Rect | null>(null);
  const last = useRef<{ shape: Shape; mode: Mode } | null>(null);
  const seq = useRef(0);
  const submitting = useRef(false);

  const session = s.session ?? "";
  const status = useStore((x) => x.sessions[session]?.status);
  const events = useStore((x) => x.events[session]);
  const activity = useStore((x) => x.activity[session]);
  const approvals = useStore((x) => x.approvals);
  const oldest = useMemo(() => Object.values(approvals).filter((a) => !a.resolved).sort((a, b) => a.request - b.request)[0], [approvals]);
  const approval = s.approval === undefined ? undefined : approvals[s.approval];

  useEffect(() => {
    const un = onDockHotkey(() => dispatch({ t: "hotkey" }));
    const key = savedShortcut();
    if (key !== DEFAULT_SHORTCUT) dockShortcut(key).catch(() => {});
    return () => { un.then((f) => f()); };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape" && !e.defaultPrevented) dispatch({ t: "esc" }); };
    addEventListener("keydown", onKey);
    return () => removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (oldest) { if (s.approval !== oldest.request) dispatch({ t: "approval", request: oldest.request }); }
    else if (s.approval !== undefined) dispatch({ t: "resolved", request: s.approval });
  }, [oldest?.request, s.approval]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (status === "done") dispatch({ t: "done" });
    else if (status === "failed") dispatch({ t: "failed" });
  }, [status, s.mode]);

  useEffect(() => {
    if (s.mode !== "done") return;
    const t = setTimeout(() => dispatch({ t: "timeout" }), 4000);
    return () => clearTimeout(t);
  }, [s.mode]);

  const prev = useRef<Mode>("hidden");
  useEffect(() => {
    if (prev.current === "prompt" && s.mode === "hidden") dockRestoreFocus().catch(() => {});
    prev.current = s.mode;
  }, [s.mode]);

  const hasApproval = s.approval !== undefined;
  useEffect(() => {
    const me = ++seq.current;
    const live = () => seq.current === me;
    (async () => {
      if ((s.mode !== "hidden" || hasApproval) && !placed.current) work.current = await dockMonitor();
      const w = work.current;
      if (!live() || !w) return;
      const next = shape(s.mode, hasApproval, w, cardH);
      const nb = bounds(next);

      if (!nb) {
        if (!placed.current) return;
        setView((v) => v && { ...v, leaving: true });
        await wait(200);
        if (!live()) return;
        await dockHide();
        placed.current = null;
        last.current = null;
        setView(null);
        return;
      }

      const before = placed.current;
      const win = before ? union(before, nb) : nb;
      if (!sameRect(win, before)) await dockPlace(win);
      placed.current = win;
      if (!live()) return;
      const view0: View = { origin: win, shape: next, mode: s.mode, scale: w.scale, animating: false, leaving: false };
      // Re-anchor the old shape to the new window origin without a transition, then morph.
      setView(before && last.current ? { ...view0, ...last.current } : view0);
      last.current = { shape: next, mode: s.mode };
      if (before) await frames();
      if (!live()) return;
      if (before) setView({ ...view0, animating: true });
      await dockShow(s.mode === "prompt" || s.mode === "expanded");
      await wait(MS);
      if (!live() || sameRect(win, nb)) return;
      await dockPlace(nb);
      placed.current = nb;
      if (!live()) return;
      setView((v) => v && { ...v, origin: nb, animating: false });
    })().catch(() => {});
  }, [s.mode, hasApproval, cardH]);

  const submit = async (text: string) => {
    if (submitting.current) return;
    submitting.current = true;
    try {
      const target = await dockTarget().catch(() => null);
      const id = ask(target ? `[The user was in window ${target.window} "${target.title}" when asking.]\n${text}` : text, null);
      dockRestoreFocus().catch(() => {});
      dispatch({ t: "submit", session: id });
    } finally { submitting.current = false; }
  };

  if (!view || !view.shape.base) return <><Sprite /><div className="dock-root" /></>;
  const { origin: o, scale: k, shape: sh } = view;
  const at = (r: Rect) => ({ transform: `translate(${(r.x - o.x) / k}px, ${(r.y - o.y) / k}px)` });
  const flags = { "data-anim": view.animating || undefined, "data-leaving": view.leaving || undefined };

  const tab = (kind: TabKind, line: string) => (
    <Tab kind={kind} line={line} onOpen={kind === "waiting" ? undefined : () => dispatch({ t: "click_tab" })}
      onStop={kind === "working" ? () => { cancel(session); dispatch({ t: "cancel" }); } : undefined} onClose={kind === "failed" ? () => dispatch({ t: "esc" }) : undefined} />
  );
  const failure = latest(events, "failed");
  const body = {
    hidden: tab("waiting", approval?.summary ?? ""),
    prompt: <PromptBox onSubmit={submit} />,
    working: tab("working", activity ?? ""),
    done: tab("done", firstLine(events)),
    failed: tab("failed", failure?.error ?? ""),
    expanded: s.session ? <Expanded session={s.session} onClose={() => dispatch({ t: "esc" })} /> : null,
  }[view.mode];
  const kind = view.mode === "prompt" ? "prompt" : view.mode === "expanded" ? "panel" : "tab";

  return (
    <>
      <Sprite />
      <div className="dock-root">
        <div className="dock-shell" data-shape={kind} {...flags}
          style={{ ...at(sh.base), width: sh.base.w / k, height: sh.base.h / k }}>
          <div key={view.mode} className="dock-fade">{body}</div>
        </div>
        {sh.card && approval && (
          <Card at={at(sh.card)} width={sh.card.w / k} anim={flags} onHeight={setCardH}>
            <ApprovalCard a={approval} newest={false} />
          </Card>
        )}
      </div>
    </>
  );
}

function Card({ at, width, anim, onHeight, children }: { at: object; width: number; anim: object; onHeight: (h: number) => void; children: ReactNode }) {
  const el = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const ro = new ResizeObserver(() => onHeight(Math.min(CARD_H, Math.ceil(el.current?.offsetHeight ?? CARD_H))));
    ro.observe(el.current!);
    return () => ro.disconnect();
  }, [onHeight]);
  return <div ref={el} className="dock-shell" data-shape="card" {...anim} style={{ ...at, width, maxHeight: CARD_H }}>{children}</div>;
}
