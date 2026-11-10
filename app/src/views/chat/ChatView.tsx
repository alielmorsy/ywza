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

  const root = useRef<HTMLDivElement>(null);
  const history = useRef<HTMLElement>(null);
  const [available, setAvailable] = useState(0);
  const [historyOpen, setHistoryOpen] = useState(true);
  const [historyDrawer, setHistoryDrawer] = useState(false);
  const [historyWidth, setHistoryWidth] = useState(() => { try { return Math.max(160, Math.min(300, Number(localStorage.getItem("ywza.historyWidth")) || 210)); } catch { return 210; } });
  const [showWindow, setShowWindow] = useState(false);
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) => setAvailable(entry.contentRect.width));
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  const showHistory = historyOpen && !ui.split && available >= historyWidth + 468;
  const sideBySide = ui.split && available >= MIN * 2 + GRIP;
  const area = useRef<HTMLDivElement>(null);
  const left = useRef<HTMLElement>(null);
  const [width, setWidth] = useState<number | null>(readWidth);

  const block = liveBlock(device);
  const latest = useRef(width);
  useLayoutEffect(() => {
    if (sideBySide && width == null && left.current) setWidth(left.current.offsetWidth);
  }, [sideBySide, width]);
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
    <div ref={root} className="relative flex h-full min-w-0">
      {(showHistory || historyDrawer) && !ui.split && <>
        <aside ref={history} className={showHistory ? "flex min-h-0 flex-none" : "absolute inset-y-0 left-0 z-20 flex border-r border-line bg-chrome shadow-lg"} style={{ width: historyWidth }}>
          <div className="flex min-h-0 w-full flex-col"><button className="btn ghost sm m-2" onClick={() => { setHistoryOpen(false); setHistoryDrawer(false); }}>Hide history</button><SessionList dev={dev} onPick={() => setHistoryDrawer(false)} /></div>
        </aside>
        {showHistory && <Splitter label="Resize history" left={history} min={160} max={300} value={historyWidth} onSize={(v) => { const n = Math.max(160, Math.min(300, v)); setHistoryWidth(n); try { localStorage.setItem("ywza.historyWidth", String(n)); } catch {} }} onCommit={() => {}} />}
      </> }
      <div ref={area} className="flex min-h-0 min-w-0 flex-1">
        <section ref={left} className="flex min-h-0 min-w-0 flex-col" hidden={ui.split && !sideBySide && showWindow}
          style={sideBySide ? { width: width ?? "50%", minWidth: MIN, maxWidth: `calc(100% - ${MIN + GRIP}px)`, flex: "none" } : { flex: "1 1 0" }}>
          <header className="flex min-h-10 flex-none flex-wrap items-center gap-1.5 py-1 border-b border-line-soft pr-2.5 pl-3.5">
            <button className="btn ghost sm" aria-expanded={showHistory || historyDrawer} onClick={() => { setUi(dev, { split: false }); if (available < historyWidth + 468) setHistoryDrawer(!historyDrawer); else setHistoryOpen(!showHistory); }}>History</button>
            <h2 className="min-w-0 flex-1 truncate">{title}</h2>
            <button className="btn sm aria-disabled:cursor-not-allowed aria-disabled:opacity-40" aria-disabled={!!block}
              title={block || "Open the live view with this chat docked"} onClick={openLive}>
              <Icon name="eye" />View desktop
            </button>
            <button className="btn sm aria-pressed:border-line-hover aria-pressed:bg-surface-3" aria-pressed={ui.split}
              title="Show the window the agent is working in beside the chat" onClick={() => { setUi(dev, { split: !ui.split }); setShowWindow(true); }}>
              <Icon name="split" />Follow agent
            </button>
          </header>
          {ui.split && !sideBySide && <button className="btn sm m-2 self-start" onClick={() => setShowWindow(true)}>Follow agent</button>}
          <ChatPane dev={dev} />
        </section>
        {ui.split && (
          <>
            {sideBySide && <Splitter left={left} onSize={resize} onCommit={commit} value={width == null ? undefined : Math.round(width)} min={MIN} max={max} />}
            <section hidden={!sideBySide && !showWindow} className="flex min-h-0 min-w-0 flex-1 flex-col">
              <header className="flex h-10 flex-none items-center gap-3 border-b border-line-soft pr-3.5 pl-3">
                <button className="btn sm" onClick={() => { setShowWindow(false); if (sideBySide) setUi(dev, { split: false }); }}>Back to chat</button>
                <span className={`min-w-0 flex-1 truncate font-semibold ${winTitle ? "" : "text-text-3"}`}>{winTitle ?? (win ? "Window" : "No window yet")}</span>
                <details className="relative"><summary className="btn ghost sm">Inspect</summary><label className="absolute right-0 top-full z-10 flex w-40 rounded-md border border-line bg-chrome p-2 cursor-pointer items-center gap-1.5 text-xs text-text-2 hover:text-text">
                  <input type="checkbox" className="size-3.5 accent-accent" checked={ui.debug} onChange={(e) => setUi(dev, { debug: e.target.checked })} />
                  Show elements
                </label></details>
              </header>
              <div className="min-h-0 flex-1"><WindowLive dev={dev} window={win} debug={ui.debug} active={active && ui.split && (sideBySide || showWindow)} /></div>
            </section>
          </>
        )}
      </div>
    </div>
  );
}
