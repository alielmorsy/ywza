import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { Icon } from "../../Icon";
import { useStore, type SEvent } from "../../api";
import { openDevice, setUi, uiOf, useNav } from "../../nav";
import { liveBlock } from "../devices";
import { WindowLive } from "../live/WindowLive";
import { ChatPane } from "./ChatPane";
import { SessionList } from "./SessionList";
import { Splitter } from "./Splitter";
import { agentWindow } from "./util";

const KEY = "ywza.chatSplit";
const MIN = 320;
const GRIP = 8;
const readWidth = () => { try { return Number(localStorage.getItem(KEY)) || null; } catch { return null; } };

export function ChatView({ dev, active }: { dev: string; active: boolean }) {
  const ui = useNav((n) => uiOf(n, dev));
  const device = useStore((s) => s.devices[dev]);
  const title = useStore((s) => (ui.session && s.sessions[ui.session]?.title) || "New chat");
  const events = useStore((s): SEvent[] | undefined => (ui.session ? s.events[ui.session] : undefined));
  const win = useMemo(() => agentWindow(events), [events]);
  const winTitle = useStore((s) => (win ? s.devices[dev]?.windows[win]?.title : undefined));

  const area = useRef<HTMLDivElement>(null);
  const left = useRef<HTMLElement>(null);
  const [width, setWidth] = useState<number | null>(readWidth);

  const block = liveBlock(device);
  const latest = useRef(width);
  useLayoutEffect(() => {
    if (ui.split && width == null && left.current) setWidth(left.current.offsetWidth);
  }, [ui.split, width]);
  const openLive = () => { if (block) return; openDevice(dev, "live"); setUi(dev, { dock: true }); };
  const resize = (px: number) => {
    const max = (area.current?.offsetWidth ?? 0) - MIN - GRIP;
    const w = Math.max(MIN, Math.min(px, max));
    latest.current = w;
    setWidth(w);
  };
  const commit = () => { try { if (latest.current) localStorage.setItem(KEY, String(Math.round(latest.current))); } catch { /* private mode */ } };
  const max = Math.max(MIN, (area.current?.offsetWidth ?? 0) - MIN - GRIP);

  return (
    <div className="grid h-full grid-cols-[210px_minmax(0,1fr)]">
      <SessionList dev={dev} />
      <div ref={area} className="flex min-h-0 min-w-0">
        <section ref={left} className="flex min-h-0 min-w-0 flex-col"
          style={ui.split ? { width: width ?? "50%", minWidth: MIN, maxWidth: `calc(100% - ${MIN + GRIP}px)`, flex: "none" } : { flex: "1 1 0" }}>
          <header className="flex h-10 flex-none items-center gap-1.5 border-b border-line-soft pr-2.5 pl-3.5">
            <h2 className="min-w-0 flex-1 truncate">{title}</h2>
            <button className="btn sm aria-disabled:cursor-not-allowed aria-disabled:opacity-40" aria-disabled={!!block}
              title={block || "Open the live view with this chat docked"} onClick={openLive}>
              <Icon name="eye" />Open live
            </button>
            <button className="btn sm aria-pressed:border-line-hover aria-pressed:bg-surface-3" aria-pressed={ui.split}
              title="Show the window the agent is working in beside the chat" onClick={() => setUi(dev, { split: !ui.split })}>
              <Icon name="split" />Split
            </button>
          </header>
          <ChatPane dev={dev} />
        </section>
        {ui.split && (
          <>
            <Splitter left={left} onSize={resize} onCommit={commit} value={width == null ? undefined : Math.round(width)} min={MIN} max={max} />
            <section className="flex min-h-0 min-w-[320px] flex-1 flex-col">
              <header className="flex h-10 flex-none items-center gap-3 border-b border-line-soft pr-3.5 pl-3">
                <span className={`min-w-0 flex-1 truncate font-semibold ${winTitle ? "" : "text-text-3"}`}>{winTitle ?? (win ? "Window" : "No window yet")}</span>
                <label className="flex flex-none cursor-pointer items-center gap-1.5 text-xs text-text-2 hover:text-text">
                  <input type="checkbox" className="size-3.5 accent-accent" checked={ui.debug} onChange={(e) => setUi(dev, { debug: e.target.checked })} />
                  Show elements
                </label>
              </header>
              <div className="min-h-0 flex-1"><WindowLive dev={dev} window={win} debug={ui.debug} active={active && ui.split} /></div>
            </section>
          </>
        )}
      </div>
    </div>
  );
}
