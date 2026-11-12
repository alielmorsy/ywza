import { useEffect, useState } from "react";
import { removeDevice, useStore } from "../api";
import { rename, setMenu, toast, useLabel, useM } from "./store";

/** Rename (shown on this phone only) or remove a paired device. Opens on long-press of a tile. */
export function DeviceMenu() {
  const name = useM((m) => m.menu);
  const connected = useStore((s) => !!name && !!s.devices[name]?.connected);
  const L = useLabel();
  const [text, setText] = useState("");
  const [sure, setSure] = useState(false);
  useEffect(() => { setText(name ? L(name) : ""); setSure(false); }, [name]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (!name) return;
    const k = (e: KeyboardEvent) => { if (e.key === "Escape") setMenu(null); };
    addEventListener("keydown", k);
    return () => removeEventListener("keydown", k);
  }, [name]);
  if (!name) return null;

  const remove = () => removeDevice(name).catch((e) => toast(`Could not remove ${name}: ${e}`));
  return (
    <div className="absolute inset-0 z-50 flex items-end bg-backdrop/70" onClick={() => setMenu(null)}>
      <div role="dialog" aria-modal="true" aria-label={`Manage ${L(name)}`} onClick={(e) => e.stopPropagation()}
        className="m-rise flex w-full flex-col gap-3 rounded-t-2xl border-t border-line bg-ground px-5 pt-5 pb-[calc(env(safe-area-inset-bottom)+16px)]">
        <div>
          <h2 className="text-base">{L(name)}</h2>
          <p className="text-xs text-text-2">{connected ? "Online" : "Offline"}{L(name) !== name && ` · paired as ${name}`}</p>
        </div>
        <label className="flex flex-col gap-1.5">
          <span className="text-xs text-text-2">Display name on this phone</span>
          <input value={text} maxLength={32} autoComplete="off" onChange={(e) => setText(e.target.value)}
            className="h-12 rounded-[10px] border border-line bg-chrome px-3 outline-none focus:border-accent focus:shadow-[0_0_0_3px_var(--c-accent-soft)]" />
        </label>
        <div className="flex gap-2">
          <button className="m-btn flex-1" onClick={() => setMenu(null)}>Cancel</button>
          <button className="m-btn primary flex-1" onClick={() => { rename(name, text); setMenu(null); }}>Save name</button>
        </div>
        <button className="m-btn danger" onClick={() => (sure ? remove() : setSure(true))}>{sure ? `Tap again to unpair ${name}` : "Unpair this device"}</button>
      </div>
    </div>
  );
}
