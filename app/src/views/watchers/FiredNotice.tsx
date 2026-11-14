import { callTool, dismissFired, reportError, useStore } from "../../api";
import { Icon } from "../../Icon";

/** Strip under a device's header for a watcher that fired. "Turn off" stops a trigger that keeps firing. */
export function FiredNotice({ dev }: { dev: string }) {
  const f = useStore((s) => s.devices[dev]?.fired?.[0]);
  const canOff = useStore((s) => s.devices[dev]?.permissions?.tools !== false);
  if (!f) return null;
  const off = async () => {
    const r = await callTool(dev, "watcher_set", { id: f.watcher, enabled: false });
    if (r.ok) dismissFired(dev, f.watcher); else reportError(dev, r.text);
  };
  return (
    <div role="status" className="flex flex-none items-center gap-2 border-b border-line-soft bg-surface-2 px-4 py-1.5 text-xs">
      <Icon name="eye" className="text-accent" />
      <span className="min-w-0 flex-1 truncate"><b className="font-medium">{f.name}</b> fired: {f.text}</span>
      {canOff && <button className="btn sm" onClick={off}>Turn off</button>}
      <button className="btn ghost sm px-[7px]" aria-label={`Dismiss ${f.name}`} onClick={() => dismissFired(dev, f.watcher)}><Icon name="x" /></button>
    </div>
  );
}
