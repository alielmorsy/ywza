import { useState } from "react";
import { useStore } from "../../api";
import { Icon } from "../../Icon";
import { Dialog } from "../Dialog";
import { deviceLabel } from "../devices";
import { WatcherList } from "./WatcherList";

/** Header button of a device that opens its watchers. Needs the `tools` permission on that device. */
export function WatchersButton({ dev }: { dev: string }) {
  const [open, setOpen] = useState(false);
  const d = useStore((s) => s.devices[dev]);
  const off = !d?.connected ? "Offline" : d.permissions?.tools === false ? "This device does not let you run its tools" : null;
  return (
    <>
      <button className="btn ghost sm" aria-disabled={!!off} title={off ?? "Watchers: triggers that run while you are away"} onClick={() => { if (!off) setOpen(true); }}>
        <Icon name="eye" />Watchers
      </button>
      <Dialog open={open} onClose={() => setOpen(false)} label={`Watchers on ${deviceLabel(dev)}`} className="w-[560px]">
        <div className="flex max-h-[70vh] flex-col gap-3 p-4">
          <h2 className="text-sm">Watchers on {deviceLabel(dev)}</h2>
          <div className="min-h-0 overflow-auto"><WatcherList dev={dev} /></div>
          <div className="flex justify-end"><button className="btn" onClick={() => setOpen(false)}>Close</button></div>
        </div>
      </Dialog>
    </>
  );
}
