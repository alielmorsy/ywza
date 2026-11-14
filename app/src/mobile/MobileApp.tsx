import { useEffect } from "react";
import { send, useStore } from "../api";
import { Icon, Sprite, type IconName } from "../Icon";
import { Banner } from "./Banner";
import { DeviceMenu } from "./DeviceMenu";
import { FiredBanner } from "./Fired";
import { Inbox, usePendingCount } from "./Inbox";
import { Live } from "./Live";
import { Pair } from "./Pair";
import { RunDetail } from "./RunDetail";
import { Runs } from "./RunList";
import { ShareSheet } from "./Share";
import { Sheet } from "./Sheet";
import { Wall } from "./Wall";
import { useRuns } from "./runs";
import { setTab, useM, type Tab } from "./store";
import { Toast } from "./ui";

const TABS: { id: Tab; label: string; icon: IconName }[] = [
  { id: "wall", label: "Wall", icon: "grid" },
  { id: "runs", label: "Runs", icon: "list" },
  { id: "inbox", label: "Inbox", icon: "inbox" },
];

export default function MobileApp() {
  const tab = useM((m) => m.tab);
  const runId = useM((m) => m.run);
  const pairing = useM((m) => m.pairing);
  const runs = useRuns();
  const pending = usePendingCount();
  const run = runs.find((r) => r.id === runId);
  // Devices a driver may use, by the driver's own names: asked again whenever a driver connects.
  const drivers = useStore((s) => Object.values(s.devices).filter((d) => d.connected && d.llm).map((d) => d.name).sort().join("\n"));

  useEffect(() => { drivers.split("\n").filter(Boolean).forEach((n) => send(n, { type: "list_devices" })); }, [drivers]);
  useEffect(() => { // env(safe-area-inset-*) needs viewport-fit=cover
    document.querySelector('meta[name="viewport"]')?.setAttribute("content", "width=device-width, initial-scale=1, viewport-fit=cover");
  }, []);

  return (
    <div className="m-root">
      <Sprite />
      <Banner />
      <FiredBanner />
      <main className="relative flex min-h-0 flex-1 flex-col">
        {tab === "wall" && <Wall />}
        {tab === "runs" && (run ? <RunDetail run={run} /> : <Runs />)}
        {tab === "inbox" && <Inbox />}
      </main>
      <nav aria-label="Sections" className="z-40 flex h-[var(--tabs-h)] flex-none border-t border-line-soft bg-chrome pb-[env(safe-area-inset-bottom)]">
        {TABS.map((t) => {
          const on = tab === t.id;
          return (
            <button key={t.id} type="button" aria-current={on ? "page" : undefined} onClick={() => setTab(t.id)}
              aria-label={t.id === "inbox" && pending ? `Inbox, ${pending} waiting` : t.label}
              className={`relative flex flex-1 flex-col items-center justify-center gap-0.5 text-[11px] ${on ? "text-text" : "text-text-2"}`}>
              <Icon name={t.icon} className={`!size-[22px] ${on ? "text-primary" : ""}`} />
              {t.label}
              {t.id === "inbox" && pending > 0 && (
                <span className="absolute top-1.5 left-[calc(50%+6px)] grid h-4 min-w-4 place-items-center rounded-full bg-warn px-1 text-[10px] font-bold text-ink">{pending}</span>
              )}
            </button>
          );
        })}
      </nav>
      <Live />
      <Sheet />
      {pairing && <Pair />}
      <DeviceMenu />
      <ShareSheet />
      <Toast />
    </div>
  );
}
