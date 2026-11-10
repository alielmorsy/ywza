import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LOCAL, useStore, waitingOn } from "./api";
import { flash, openDevice, select, setPairing, useNav, type View } from "./nav";
import { Icon, Sprite } from "./Icon";
import { Rail } from "./views/Rail";
import { DeviceView } from "./views/DeviceView";
import Settings from "./views/Settings";
import { PairDialog } from "./views/PairDialog";
import { ServiceDown } from "./views/ServiceDown";
import { deviceLabel, liveBlock, useDeviceOrder } from "./views/devices";
import { takeSaved } from "./views/localConfig";

const VIEW_KEYS: Record<string, View> = { KeyC: "chat", KeyL: "live", KeyA: "access" };

const winBtn = "grid w-[46px] place-items-center text-text-2 transition-colors hover:bg-surface-2 hover:text-text";

export default function App() {
  const selected = useNav((n) => n.selected);
  const flashed = useNav((n) => n.status);
  const pairOpen = useNav((n) => n.pairing);
  const auto = useNav((n) => n.approval === "auto");
  const waiting = useStore((s) => waitingOn(s));
  const status = waiting && flashed.tone !== "bad" ? { text: `${deviceLabel(waiting)}: waiting for approval`, tone: "warn" as const, pulse: true } : flashed;
  const devices = useStore((s) => s.devices);
  const error = useStore((s) => s.error);
  const config = useStore((s) => s.devices[LOCAL]?.config);
  const order = useDeviceOrder();
  const [visited, setVisited] = useState<string[]>([]);
  // Grace period at startup so the first connect doesn't flash the "down" card.
  const [booted, setBooted] = useState(false);
  useEffect(() => { const t = setTimeout(() => setBooted(true), 1500); return () => clearTimeout(t); }, []);

  if (!visited.includes(selected)) setVisited([...visited, selected]);

  useEffect(() => {
    if (!error) return;
    takeSaved();
    flash(error.device === LOCAL ? error.message : `${error.device}: ${error.message}`, "bad");
  }, [error]);

  useEffect(() => { const m = takeSaved(); if (m) flash(m); }, [config]);

  useEffect(() => { if (selected !== "settings" && !devices[selected]) select(LOCAL); }, [selected, devices]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.altKey || document.querySelector("dialog[open]")) return;
      if (e.key === ",") { e.preventDefault(); select("settings"); return; }
      if (!e.shiftKey && /^[1-9]$/.test(e.key)) {
        const name = order[Number(e.key) - 1];
        if (name) { e.preventDefault(); select(name); }
        return;
      }
      const view = e.shiftKey ? VIEW_KEYS[e.code] : undefined;
      if (!view) return;
      e.preventDefault();
      if (selected === "settings") return;
      if (view === "access" && selected === LOCAL) return;
      if (view === "live" && liveBlock(devices[selected])) return;
      openDevice(selected, view);
    };
    addEventListener("keydown", onKey);
    return () => removeEventListener("keydown", onKey);
  }, [order, selected, devices]);

  const paired = order.slice(1);
  const online = paired.filter((n) => devices[n]?.connected).length;
  const local = devices[LOCAL];
  const win = getCurrentWindow();
  const down = !local?.connected && booted;

  return (
    <div className="grid h-full bg-ground grid-rows-[32px_1fr_24px]">
      <Sprite />
      <header className="flex items-center border-b border-line-soft bg-chrome pl-3" data-tauri-drag-region>
        <span className="pointer-events-none font-semibold tracking-wide text-text">ywza</span>
        <span className="h-full flex-1" data-tauri-drag-region />
        <div className="flex h-full">
          <button className={winBtn} title="Minimize" aria-label="Minimize" onClick={() => win.minimize()}><Icon name="min" className="!size-3" /></button>
          <button className={winBtn} title="Maximize" aria-label="Maximize" onClick={() => win.toggleMaximize()}><Icon name="max" className="!size-3" /></button>
          <button className={`${winBtn} hover:!bg-bad hover:!text-ink`} title="Close" aria-label="Close" onClick={() => win.close()}><Icon name="x" className="!size-3" /></button>
        </div>
      </header>

      <div className="grid min-h-0 grid-cols-[clamp(168px,22vw,200px)_minmax(0,1fr)]">
        <Rail onPair={() => setPairing(true)} />
        <main className="relative min-w-0 overflow-hidden">
          {down && selected !== "settings" && <ServiceDown detail={local?.detail} />}
          {visited.map((name) => {
            const body = name === "settings" ? <Settings /> : devices[name] ? <DeviceView dev={name} selected={selected === name} /> : null;
            return body && <div key={name} className="absolute inset-0" hidden={selected !== name || (down && name !== "settings")}>{body}</div>;
          })}
        </main>
      </div>

      <footer className="flex items-center gap-4 border-t border-line-soft bg-chrome px-3 text-xs text-text-2">
        <div className="flex items-center gap-2 text-text" aria-live="polite">
          <span className={`dot ${status.tone} ${status.pulse ? "pulse" : ""}`} />
          <span>{status.text}</span>
        </div>
        <span className="flex-1" />
        {auto && <><span className="flex items-center gap-1.5 text-warn" title="Chats run tools without asking. Change it under the message box."><Icon name="shield" className="!size-3.5" />Auto-approve on</span><span className="h-3 w-px bg-line" /></>}
        <span>{paired.length ? `${paired.length} paired, ${online} online` : "No paired devices"}</span>
        <span className="h-3 w-px bg-line" />
        <span title={local?.connected ? undefined : local?.detail}>{local?.connected ? "Service running" : "Service not running"}</span>
      </footer>

      <PairDialog open={pairOpen} onClose={() => setPairing(false)} />
    </div>
  );
}
