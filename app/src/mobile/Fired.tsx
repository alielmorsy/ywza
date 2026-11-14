import { useMemo, useState } from "react";
import { callTool, dismissFired, useStore, type Fired } from "../api";
import { Icon } from "../Icon";
import { toast, useLabel } from "./store";
import { DevChip } from "./ui";

export type FiredItem = Fired & { device: string };

/** Watchers that fired on any paired device, newest first. */
export function useFired(): FiredItem[] {
  const devices = useStore((s) => s.devices);
  return useMemo(() => Object.values(devices).flatMap((d) => (d.fired ?? []).map((f) => ({ ...f, device: d.name }))).sort((a, b) => b.at - a.at), [devices]);
}

/** A watcher fired. "Turn off" is the kill switch for a trigger that keeps firing. */
export function FiredCard({ f, className = "" }: { f: FiredItem; className?: string }) {
  const L = useLabel();
  const canOff = useStore((s) => s.devices[f.device]?.permissions?.tools !== false);
  const [busy, setBusy] = useState(false);
  const off = async () => {
    setBusy(true);
    const r = await callTool(f.device, "watcher_set", { id: f.watcher, enabled: false });
    setBusy(false);
    toast(r.ok ? `Turned off ${f.name}` : r.text);
    if (r.ok) dismissFired(f.device, f.watcher);
  };
  return (
    <div role="group" aria-label={`Watcher fired: ${f.name}`} className={`flex flex-col gap-2 rounded-xl border border-accent/50 bg-surface-2 p-3 ${className}`}>
      <div className="flex min-w-0 items-center gap-2 text-[13px]">
        <Icon name="eye" className="text-accent" />
        <b className="min-w-0 flex-1 truncate font-semibold">{f.name}</b>
        <DevChip name={f.device} />
        <button type="button" className="m-btn ghost -mr-2 size-9 !min-h-9 !p-0" aria-label={`Dismiss ${f.name}`} onClick={() => dismissFired(f.device, f.watcher)}><Icon name="x" className="!size-3" /></button>
      </div>
      <p className="break-words select-text text-[13px]">{f.text}</p>
      {canOff && (
        <div className="flex gap-2">
          <button type="button" className="m-btn sm" disabled={busy} aria-label={`Turn off ${f.name} on ${L(f.device)}`} onClick={off}>Turn off</button>
        </div>
      )}
    </div>
  );
}

/** The latest fired watcher above the tabs' content; the rest wait in the inbox. */
export function FiredBanner() {
  const fired = useFired();
  if (!fired.length) return null;
  return (
    <div className="m-rise mx-3 mb-2 flex-none">
      <FiredCard f={fired[0]} />
      {fired.length > 1 && <p className="mt-1 px-1 text-[11px] text-text-2">{fired.length - 1} more in the inbox</p>}
    </div>
  );
}
