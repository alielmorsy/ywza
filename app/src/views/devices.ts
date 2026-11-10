import { useMemo } from "react";
import { LOCAL, useStore, type Device } from "../api";

export const deviceLabel = (name: string) => (name === LOCAL ? "This PC" : name);

export const statusText = (d?: Device) => (d?.connected ? "Online" : d?.detail || "Offline");

export const isConnecting = (d?: Device) => !d?.connected && /^connecting/i.test(d?.detail ?? "");

/** Online = filled dot, offline = ring, connecting = pulsing ring. */
export const dotClass = (d?: Device) => (d?.connected ? "" : isConnecting(d) ? "off pulse" : "off");

/** Why the live view cannot be opened for `d`, or null when it can. */
export const liveBlock = (d?: Device) =>
  !d?.connected ? "Offline" : d.permissions?.view === false ? "This device does not let you view its screen" : null;

/** Rail order: This PC, then paired devices with online ones first. */
export function useDeviceOrder(): string[] {
  const devices = useStore((s) => s.devices);
  return useMemo(() => {
    const rest = Object.values(devices)
      .filter((d) => d.name !== LOCAL)
      .sort((a, b) => Number(b.connected) - Number(a.connected) || a.name.localeCompare(b.name));
    return [LOCAL, ...rest.map((d) => d.name)];
  }, [devices]);
}
