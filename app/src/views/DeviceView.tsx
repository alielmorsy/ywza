import { useState, type ReactNode } from "react";
import { LOCAL, useStore } from "../api";
import { setUi, uiOf, useNav, type View } from "../nav";
import { Access } from "./Access";
import { ChatView } from "./chat/ChatView";
import { LiveView } from "./live/LiveView";
import { deviceLabel, dotClass, liveBlock, statusText } from "./devices";

const Pane = ({ on, children }: { on: boolean; children: ReactNode }) => (
  <div className="absolute inset-0 overflow-hidden" hidden={!on}>{children}</div>
);

/** One device: header with the view switch, then chat and live kept mounted once opened. */
export function DeviceView({ dev, selected }: { dev: string; selected: boolean }) {
  const d = useStore((s) => s.devices[dev]);
  const ui = useNav((n) => uiOf(n, dev));
  const [liveSeen, setLiveSeen] = useState(false);

  const local = dev === LOCAL;
  const liveOff = liveBlock(d);
  const view: View = ui.view === "access" && local ? "chat" : ui.view === "live" && liveOff ? "chat" : ui.view;
  if (view === "live" && !liveSeen) setLiveSeen(true);

  const segs: { v: View; label: string; key: string; off: string | null }[] = [
    { v: "chat", label: "Chat", key: "C", off: null },
    { v: "live", label: "View desktop", key: "L", off: liveOff },
    ...(local ? [] : [{ v: "access" as View, label: "Access", key: "A", off: null }]),
  ];

  return (
    <div className="flex h-full flex-col">
      <header className="flex h-10 flex-none items-center gap-3 border-b border-line-soft px-4">
        <h1 className="min-w-0 truncate">{deviceLabel(dev)}</h1>
        <span className="flex min-w-0 items-center gap-2 text-xs text-text-2" title={d?.version ? `ywza ${d.version}` : undefined}>
          <span className={`dot ${dotClass(d)}`} />
          <span className="truncate">{statusText(d)}</span>
        </span>
        <span className="flex-1" />
        <div className="seg" role="group" aria-label="View">
          {segs.map((s) => (
            <button key={s.v} aria-pressed={view === s.v} aria-disabled={!!s.off} title={s.off ?? `${s.label} (Ctrl+Shift+${s.key})`}
              onClick={() => { if (!s.off) setUi(dev, { view: s.v }); }}>
              {s.label}
            </button>
          ))}
        </div>
      </header>
      <div className="relative min-h-0 flex-1">
        <Pane on={view === "chat"}><ChatView dev={dev} active={selected && view === "chat"} /></Pane>
        {liveSeen && <Pane on={view === "live"}><LiveView dev={dev} active={selected && view === "live"} /></Pane>}
        {view === "access" && <Pane on><Access dev={dev} /></Pane>}
      </div>
    </div>
  );
}
