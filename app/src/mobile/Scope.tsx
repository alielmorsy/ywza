import { useState } from "react";
import { useStore } from "../api";
import { Icon } from "../Icon";
import { canDrive, devColor, useDeviceNames, useDriver } from "./devices";
import { clearSel, setDriver, switchLive, toggleSel, useLabel, useM } from "./store";

/** Driver picker and the devices the driver may use; on the live view the chips switch the device on screen. */
export function Scope() {
  const L = useLabel();
  const names = useDeviceNames();
  const devices = useStore((s) => s.devices);
  const driver = useDriver();
  const sel = useM((m) => m.sel);
  const live = useM((m) => m.live);
  const [picking, setPicking] = useState(false);
  const none = !sel.length;

  if (picking) {
    return (
      <div className="m-scroll flex flex-none items-center gap-1.5 px-3 pb-1.5" role="group" aria-label="Driver">
        <em className="flex-none pr-0.5 text-[11px] text-text-2 not-italic">Driver</em>
        {names.map((n) => {
          const can = canDrive(devices[n]);
          return (
            <button key={n} className={`m-chip ${n === driver ? "on" : ""}`} disabled={!can} aria-pressed={n === driver}
              onClick={() => { setDriver(n); setPicking(false); }}>
              {L(n)}{!can && (devices[n]?.connected ? " - no assistant" : " - offline")}
            </button>
          );
        })}
      </div>
    );
  }
  return (
    <div className="m-scroll flex flex-none items-center gap-1.5 px-3 pb-1.5">
      <button className="m-chip cov" onClick={() => setPicking(true)} aria-label="Choose the driver">
        <Icon name="spark" className="!size-3.5" />{driver ? `Driver: ${L(driver)}` : "No assistant"}<Icon name="chev" className="!size-3" />
      </button>
      <em className="flex-none px-0.5 text-[11px] text-text-2 not-italic">{live ? "Watching" : "Can use"}</em>
      {!live && <button className={`m-chip ${none ? "on" : ""}`} aria-pressed={none} onClick={clearSel}>Driver only</button>}
      {names.filter((n) => live || n !== driver).map((n) => {
        const on = live ? live.dev === n : sel.includes(n);
        return (
          <button key={n} className={`m-chip ${on ? "on" : ""}`} aria-pressed={on} disabled={!devices[n]?.connected}
            onClick={() => (live ? switchLive(n) : toggleSel(n))}>
            <i className="size-1.5 rounded-full" style={{ background: devColor(n) }} />{L(n)}
          </button>
        );
      })}
    </div>
  );
}

/** Why the selected devices will not be usable, or null. */
export function useDriveHint(): string | null {
  const driver = useDriver();
  const sel = useM((m) => m.sel);
  const blocked = useStore((s) => !!driver && s.devices[driver]?.permissions?.drive === false);
  return blocked && sel.some((n) => n !== driver) ? `Turn on "Drive other devices" for this phone in ${driver}'s Access settings` : null;
}
