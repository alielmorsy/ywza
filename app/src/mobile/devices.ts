import { useMemo } from "react";
import { LOCAL, send, useStore, type Device, type State } from "../api";
import { useM } from "./store";

/** The phone's paired devices, by name. */
export function useDeviceNames(): string[] {
  const joined = useStore((s) => Object.keys(s.devices).filter((n) => n !== LOCAL).sort().join("\n"));
  return useMemo(() => (joined ? joined.split("\n") : []), [joined]);
}

/** Stable color per device, so a device keeps its color in the step trace, chips and tiles. */
export const devColor = (name: string) => `var(--d${[...name].reduce((h, c) => h + c.charCodeAt(0), 0) % 6})`;

export const canDrive = (d?: Device) => !!d?.connected && d.llm === true;

/** The chosen driver if it can drive, else the first device that can. */
export const pickDriver = (s: State, pick?: string) =>
  pick && canDrive(s.devices[pick]) ? pick : Object.keys(s.devices).sort().find((n) => n !== LOCAL && canDrive(s.devices[n]));

export function useDriver(): string | undefined {
  const pick = useM((m) => m.driver);
  return useStore((s) => pickDriver(s, pick));
}

/** Maps the phone's devices to the names `driver` knows them by (matched by node id). Unknown ones come back in `missing`. */
export function driverNames(s: State, driver: string, names: string[]) {
  const known = s.devices[driver]?.known;
  if (!known) { send(driver, { type: "list_devices" }); return { list: [] as string[], missing: names, loading: true }; }
  const list: string[] = [], missing: string[] = [];
  for (const n of names) {
    const hit = known.find((k) => k.node_id === s.devices[n]?.nodeId);
    if (hit) list.push(hit.name); else missing.push(n);
  }
  return { list, missing, loading: false };
}

/** Name of the phone's device that `name` (one of `driver`'s names) refers to. */
export function phoneName(s: State, driver: string, name?: string | null) {
  if (!name) return driver;
  const nid = s.devices[driver]?.known?.find((k) => k.name === name)?.node_id;
  return (nid && Object.values(s.devices).find((d) => d.nodeId === nid)?.name) || name;
}
