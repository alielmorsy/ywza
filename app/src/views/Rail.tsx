import { useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import { LOCAL, removeDevice, useStore, waitingOn, type Device } from "../api";
import { openDevice, select, useNav, type View } from "../nav";
import { Icon } from "../Icon";
import { ContextMenu, type MenuItem } from "./ContextMenu";
import { Confirm } from "./Dialog";
import { deviceLabel, dotClass, liveBlock, statusText, useDeviceOrder } from "./devices";

const subline = (d?: Device, self?: boolean) =>
  self ? "Same machine as This PC" : d?.connected && d.os ? `Online, ${d.os}` : statusText(d);

function Item({ name, selected, onMenu }: { name: string; selected: boolean; onMenu: (e: MouseEvent, name: string) => void }) {
  const d = useStore((s) => s.devices[name]);
  const self = useStore((s) => name !== LOCAL && !!d?.nodeId && d.nodeId === s.devices[LOCAL]?.nodeId);
  const waiting = useStore((s) => !!waitingOn(s, name));
  return (
    <button data-rail className="rail-item" aria-current={selected} onClick={() => select(name)} onContextMenu={(e) => onMenu(e, name)}
      title={`${deviceLabel(name)}, ${statusText(d).toLowerCase()}`}>
      <span className={`dot ${dotClass(d)}`} />
      <span className="min-w-0 flex-1">
        <span className="block truncate">{deviceLabel(name)}</span>
        {waiting
          ? <span className="flex items-center gap-1.5 truncate text-xs text-text"><span className="dot warn pulse" aria-hidden />Waiting for you</span>
          : <span className="block truncate text-xs text-text-3">{subline(d, self)}</span>}
      </span>
    </button>
  );
}

export function Rail({ onPair }: { onPair: () => void }) {
  const order = useDeviceOrder();
  const selected = useNav((n) => n.selected);
  const devices = useStore((s) => s.devices);
  const [menu, setMenu] = useState<{ x: number; y: number; name: string } | null>(null);
  const [removing, setRemoving] = useState<string | null>(null);
  const root = useRef<HTMLElement>(null);
  const lastRemoving = useRef("");
  if (removing) lastRemoving.current = removing;

  const keys = (e: KeyboardEvent) => {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key) || !(e.target as HTMLElement).closest("[data-rail]")) return;
    const b = [...root.current!.querySelectorAll<HTMLElement>("[data-rail]")];
    const i = b.indexOf(document.activeElement as HTMLElement);
    const next = e.key === "Home" ? 0 : e.key === "End" ? b.length - 1 : Math.min(b.length - 1, Math.max(0, i + (e.key === "ArrowDown" ? 1 : -1)));
    e.preventDefault();
    b[next]?.focus();
  };

  const items = (name: string): MenuItem[] => {
    const go = (v: View) => () => openDevice(name, v);
    return [
      { label: "Chat", run: go("chat") },
      { label: "Live", run: go("live"), disabled: liveBlock(devices[name]) ?? false },
      ...(name === LOCAL ? [] : [{ label: "Access", run: go("access") }, { label: "Remove", run: () => setRemoving(name), danger: true }]),
    ];
  };

  return (
    <nav ref={root} className="flex min-h-0 flex-col border-r border-line-soft bg-chrome" aria-label="Devices" onKeyDown={keys}>
      <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto p-2">
        {order.map((name) => (
          <Item key={name} name={name} selected={selected === name}
            onMenu={(e, n) => { e.preventDefault(); setMenu({ x: e.clientX, y: e.clientY, name: n }); }} />
        ))}
        {order.length === 1 && <p className="px-2.5 pt-2 text-xs text-text-3">No paired devices yet. Pair one to chat with it or watch its screen.</p>}
      </div>
      <div className="flex flex-col gap-0.5 border-t border-line-soft p-2">
        <button data-rail className="rail-item" onClick={onPair}><Icon name="plus" />Pair a device</button>
        <button data-rail className="rail-item" aria-current={selected === "settings"} title="Settings (Ctrl+,)" onClick={() => select("settings")}>
          <Icon name="sliders" />Settings
        </button>
      </div>
      {menu && <ContextMenu x={menu.x} y={menu.y} items={items(menu.name)} onClose={() => setMenu(null)} />}
      <Confirm open={!!removing} title={`Remove ${removing ?? lastRemoving.current}?`} action="Remove"
        body="It is unpaired and loses all access to this PC. To use it again you pair it with a new code."
        onConfirm={() => removeDevice(removing!)} onClose={() => setRemoving(null)} />
    </nav>
  );
}
