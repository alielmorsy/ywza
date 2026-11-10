import { useMemo } from "react";
import { LOCAL, useStore, type Device } from "../api";

export const deviceLabel = (name: string) => (name === LOCAL ? "This PC" : name);

/** Connection details come from the backend in lowercase ("no route to device (offline)"). */
const sentence = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

export const statusText = (d?: Device) => (d?.connected ? "Online" : d?.detail ? sentence(d.detail) : "Offline");

export const isConnecting = (d?: Device) => !d?.connected && /^connecting/i.test(d?.detail ?? "");

/** Online = filled dot, offline = ring, connecting = pulsing ring. */
export const dotClass = (d?: Device) => (d?.connected ? "" : isConnecting(d) ? "off pulse" : "off");

/** Why the live view cannot be opened for `d`, or null when it can. */
export const liveBlock = (d?: Device) =>
  !d?.connected ? "Offline" : d.permissions?.view === false ? "This device does not let you view its screen" : null;

/** Rail order: This PC, then paired devices by name. Stable, so Ctrl+1..9 keep their device. */
export function useDeviceOrder(): string[] {
  const names = useStore((s) => Object.keys(s.devices).filter((n) => n !== LOCAL).sort().join("\n"));
  return useMemo(() => [LOCAL, ...(names ? names.split("\n") : [])], [names]);
}
