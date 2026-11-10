import { useCallback, useEffect, useSyncExternalStore } from "react";
import { acquire, infoOf, setMonitor, subscribe, type Sink } from "./streams";

/**
 * Draws the device's shared live stream through `sink` while `enabled`. `monitor` asks for a monitor
 * (undefined keeps the current one). Returns the stream's current monitor and whether a frame arrived.
 */
export function useLiveStream(dev: string, sink: Sink, enabled: boolean, monitor?: number) {
  const info = useSyncExternalStore(useCallback((f) => subscribe(dev, f), [dev]), () => infoOf(dev));
  useEffect(() => { if (enabled && monitor !== undefined) setMonitor(dev, monitor); }, [dev, enabled, monitor]);
  useEffect(() => (enabled ? acquire(dev, sink) : undefined), [dev, sink, enabled]);
  return info;
}

export { setMonitor };
