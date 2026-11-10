import { useEffect, useState, type KeyboardEvent } from "react";
import { send, useStore, type Monitor } from "../../api";
import { Icon } from "../../Icon";

const title = (m: Monitor) => `${m.name}, ${m.width}x${m.height}, ${Math.round(m.scale * 100)}%`;

/** Outline whose aspect matches the monitor, fitted into 24x15. */
function Glyph({ m }: { m: Monitor }) {
  const k = Math.min(24 / m.width, 15 / m.height);
  return <span className="rounded-[2px] border-[1.5px] border-current" style={{ width: Math.round(m.width * k), height: Math.round(m.height * k) }} />;
}

export function MonitorPicker({ dev, value, onPick }: { dev: string; value: number; onPick: (index: number) => void }) {
  const list = useStore((s) => s.devices[dev]?.monitors);
  const outdated = useStore((s) => !!s.devices[dev]?.outdated);
  const [waited, setWaited] = useState(false);
  useEffect(() => { send(dev, { type: "list_monitors" }); }, [dev]);
  useEffect(() => { const t = setTimeout(() => setWaited(true), 3000); return () => clearTimeout(t); }, []);

  if (!list) return waited
    ? <span className="px-2 text-xs text-text-3">No monitors reported</span>
    : <span className="flex gap-1" aria-busy="true" aria-label="Loading monitors">
        {[0, 1].map((i) => <span key={i} className="h-6 w-10 animate-pulse rounded-md bg-surface-2" />)}
      </span>;
  if (!list.length) return <span className="px-2 text-xs text-text-3">No monitors reported</span>;

  const tabbable = list.some((m) => m.index === value) ? value : list[0].index;
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
    if (!step) return;
    e.preventDefault();
    const at = list.findIndex((m) => m.index === tabbable);
    const next = list[(at + step + list.length) % list.length];
    onPick(next.index);
    (e.currentTarget.querySelectorAll<HTMLElement>("[role=radio]")[list.indexOf(next)]).focus();
  };

  return (
    <div role="radiogroup" aria-label="Monitor" className="flex gap-0.5" onKeyDown={onKey}
      title={outdated ? "This device runs an older ywza agent, so only its main monitor is available" : undefined}>
      {list.map((m) => {
        const on = m.index === value;
        return (
          <button key={m.index} role="radio" aria-checked={on} tabIndex={m.index === tabbable ? 0 : -1} title={title(m)} aria-label={`Monitor ${m.index + 1}, ${m.width}x${m.height}${m.primary ? ", primary" : ""}`}
            onClick={() => onPick(m.index)}
            className={`flex h-6 items-center gap-1.5 rounded-md px-2 transition-colors ${on ? "bg-accent-bg text-ink" : "text-text-2 hover:bg-surface-2 hover:text-text"}`}>
            <Glyph m={m} />
            <span className="mono text-xs font-semibold">{m.index + 1}</span>
            {m.primary && <Icon name="star" className="size-2.5 fill-current" />}
          </button>
        );
      })}
    </div>
  );
}
