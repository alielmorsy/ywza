import { LOCAL, cancel, getState, prompt } from "../api";
import { driverNames, pickDriver } from "./devices";
import { getM, setFocus, toast } from "./store";
import type { Run } from "./runs";

/** Sends `text` to `run`'s driver, or starts a run on the chosen driver that may use `scope`. Returns an error to show, if any. */
export function submit(text: string, o: { run?: Run; scope: string[] }): string | undefined {
  const s = getState();
  const driver = o.run?.driver ?? pickDriver(s, getM().driver);
  if (!driver) return "No connected device has an assistant";
  if (!s.devices[driver]?.connected) return `${driver} is offline`;
  if (s.devices[driver].permissions?.chat === false) return `${driver} does not let this phone chat`;
  let devices: string[] | undefined;
  if (!o.run && o.scope.length) {
    const names = driverNames(s, driver, o.scope.filter((n) => n !== driver));
    if (names.loading) return `Still reading ${driver}'s devices, try again`;
    if (names.missing.length) toast(`${names.missing.join(", ")} not paired with ${driver}, left out`);
    devices = names.list.length ? names.list : undefined;
  }
  // A follow-up must name the same devices the run started with, or the driver refuses it.
  const own = o.run && s.remote[o.run.id];
  if (own) devices = own.devices;
  const id = prompt(own?.device ?? LOCAL, text, o.run?.id ?? null, { via: driver, devices, approval: "rules" }); // always set: the agent gates app chats only
  if (!o.run) { setFocus(id); toast(devices ? `Run started on ${driver} + ${devices.length} more` : `Run started on ${driver}`); }
}

export const stopRun = (r: Run) => cancel(r.id, r.driver);
