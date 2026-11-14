import { useCallback, useEffect, useRef, useState } from "react";
import { reportError } from "../../api";
import { setUi, uiOf, useNav } from "../../nav";
import { Icon } from "../../Icon";
import { ChatPane } from "../chat/ChatPane";
import { useAccess } from "./access";
import { drawFull } from "./draw";
import { DockPanel } from "./DockPanel";
import { FULL } from "./input";
import { useLens } from "./lens";
import { LensEditor, LensLayer } from "./LensChips";
import { MonitorPicker } from "./MonitorPicker";
import { unavailable } from "./Notice";
import { QualityMenu } from "./QualityMenu";
import type { Sink } from "./streams";
import { setMonitor, useLiveStream } from "./useLiveStream";
import { useRemoteInput } from "./useRemoteInput";

const whole = () => FULL;
const tool = (on: boolean) => `btn ghost sm px-[7px] ${on ? "bg-surface-3 text-text" : ""}`;
const Sep = () => <span className="mx-0.5 h-4 w-px bg-line" aria-hidden />;

export function LiveView({ dev, active }: { dev: string; active: boolean }) {
  const { d, name, connected, canView, canControl, canSet } = useAccess(dev);
  const dock = useNav((n) => uiOf(n, dev).dock);
  const root = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [control, setControl] = useState(false);
  const [focused, setFocused] = useState(false);
  const [full, setFull] = useState(false);

  const ok = connected && canView;
  const sink = useCallback<Sink>((f) => { if (canvas.current) drawFull(canvas.current, f); }, []);
  const { monitor, live } = useLiveStream(dev, sink, active && ok);
  const controlling = canControl && control;
  const input = useRemoteInput(dev, canvas, whole, active && ok && controlling);

  const lens = useLens(dev, { live, monitor, canAct: canControl, fail: (m) => reportError(dev, m) });
  const monitors = d?.monitors;
  useEffect(() => { // the stream may remember a monitor this device does not have
    if (active && monitors?.length && !monitors.some((m) => m.index === monitor))
      setMonitor(dev, (monitors.find((m) => m.primary) ?? monitors[0]).index);
  }, [active, monitors, monitor, dev]);
  useEffect(() => {
    const f = () => setFull(document.fullscreenElement === root.current);
    document.addEventListener("fullscreenchange", f);
    return () => document.removeEventListener("fullscreenchange", f);
  }, []);

  const off = unavailable(name, connected, canView, d?.detail);
  if (off) return off;

  const toggleFull = () => (document.fullscreenElement ? document.exitFullscreen() : root.current!.requestFullscreen()).catch(() => {});

  return (
    <div ref={root} className="flex h-full min-h-0 flex-col bg-ground">
        <div role="toolbar" aria-label="Live view" className="z-10 flex flex-none flex-wrap items-center gap-1 border-b border-line bg-chrome p-1">
          <MonitorPicker dev={dev} value={monitor} onPick={(i) => setMonitor(dev, i)} />
          <Sep />
          {canControl
            ? <button className={`${tool(controlling)} w-[88px] justify-center`} aria-pressed={controlling}
                title={controlling ? "Controlling the device. Click for view only." : "View only. Click to take control."}
                onClick={() => setControl(!control)}>
                <Icon name={controlling ? "pointer" : "eye"} />{controlling ? "Control" : "View only"}
              </button>
            : <span className="tag" title="This device does not allow control">View only</span>}
          <button className={`${tool(lens.on)} justify-center`} aria-pressed={lens.on} onClick={() => lens.setOn(!lens.on)}
            title={lens.on ? "Smart lens on: click a box to act on it. Turn off to click the screen directly." : "Smart lens: clickable boxes over the focused window"}>
            <Icon name="spark" />Lens
          </button>
          {controlling && focused && <span className="px-1 text-xs whitespace-nowrap text-text-2">Ctrl+Alt+Home returns the keyboard</span>}
          {canSet && <QualityMenu dev={dev} />}
          <button className={tool(dock)} aria-pressed={dock} title={dock ? "Hide chat" : "Show chat"} aria-label="Chat panel"
            onClick={() => setUi(dev, { dock: !dock })}><Icon name="chat" /></button>
          <button className={tool(false)} title={full ? "Exit fullscreen" : "Fullscreen"} aria-label={full ? "Exit fullscreen" : "Fullscreen"}
            onClick={toggleFull}><Icon name={full ? "shrink" : "expand"} /></button>
          <Sep />
          <span className="flex items-center gap-1.5 pr-1.5 pl-1 text-xs whitespace-nowrap text-text-2" role="status">
            <span className={`dot size-1.5 ${live ? "pulse" : "idle"}`} />{live ? "Live" : "Connecting"}
          </span>
        </div>
      <div className="flex min-h-0 flex-1">
      <div className="relative min-w-0 flex-1 overflow-hidden bg-backdrop">
        <canvas ref={canvas} aria-label={`Screen of ${name}`} tabIndex={controlling ? 0 : -1}
          className="absolute inset-0 size-full object-contain outline-none focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent"
          style={controlling ? { cursor: d?.cursor ?? "none" } : undefined} {...input}
          onFocus={() => setFocused(true)} onBlur={(e) => { setFocused(false); input.onBlur?.(e); }} />

        <LensLayer lens={lens} canvas={canvas} mon={monitors?.find((m) => m.index === monitor)} />
        {lens.editEl && (
          <LensEditor el={lens.editEl} onType={lens.type} onClose={() => lens.setEditing(null)} className="absolute top-3 left-1/2 z-10 w-[360px] max-w-[calc(100%-24px)] -translate-x-1/2" />
        )}
        {lens.active && !lens.canAct && <p className="absolute bottom-3 left-1/2 -translate-x-1/2 rounded-md border border-line bg-chrome/90 px-2.5 py-1 text-xs whitespace-nowrap text-text-2" role="status">View only: this device does not allow control</p>}

        {active && !live && (
          <div className="pointer-events-none absolute inset-0 grid place-items-center text-center" role="status">
            <div>
              <p className="text-text-2">Waiting for the screen of {name}</p>
              {d?.liveError && <p className="mt-1 max-w-[360px] text-xs text-bad">{d.liveError}</p>}
            </div>
          </div>
        )}


      </div>

      {dock && <DockPanel><ChatPane dev={dev} compact /></DockPanel>}
      </div>
    </div>
  );
}
