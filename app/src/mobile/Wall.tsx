import { useMemo } from "react";
import { useStore } from "../api";
import { useDeviceNames } from "./devices";
import { useRuns } from "./runs";
import { clearSel, openLive, setPairing, useM } from "./store";
import { tileInfo } from "./tiles";
import { useThumbs } from "./thumbs";
import { Tile } from "./Tile";
import { Empty, Header } from "./ui";

export function Wall() {
  const names = useDeviceNames();
  const devices = useStore((s) => s.devices);
  const runs = useRuns();
  const sel = useM((m) => m.sel);
  const showing = useM((m) => m.tab === "wall" && !m.live && !m.pairing);
  useThumbs(names, showing);

  const infos = useMemo(() => names.map((n) => tileInfo(n, devices[n], runs)), [names, devices, runs]);
  const count = (st: string) => infos.filter((i) => i.state === st).length;
  const sub = names.length
    ? [`${names.length} device${names.length > 1 ? "s" : ""}`, `${count("working")} working`, count("need") && `${count("need")} need you`, count("offline") && `${count("offline")} offline`].filter(Boolean).join(" · ")
    : "No devices yet";

  return (
    <section className="flex min-h-0 flex-1 flex-col" aria-label="Wall">
      <Header title="Wall" sub={sub}>
        {sel.length > 0 && <button className="m-btn sm" onClick={clearSel}>Clear scope</button>}
        <button className="m-btn sm" onClick={() => setPairing(true)}>Pair</button>
      </Header>
      <div className="m-scroll grid flex-1 grid-cols-2 content-start gap-3 px-3 pt-1 pb-[calc(var(--sheet-h)+16px)]">
        {names.map((n, i) => <Tile key={n} name={n} info={infos[i]} onOpen={(dev, from) => openLive(dev, { from })} />)}
        {!names.length && (
          <div className="col-span-2">
            <Empty title="Pair your first device">
              <p className="max-w-[280px]">Your computers show up here as live tiles. On the other device open Pair a device, then enter the 6-digit code it shows.</p>
              <button className="m-btn primary" onClick={() => setPairing(true)}>Pair a device</button>
            </Empty>
          </div>
        )}
      </div>
    </section>
  );
}
