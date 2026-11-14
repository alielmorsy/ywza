import { useState } from "react";
import { Icon } from "../../Icon";
import { Tog } from "../Form";
import { describeThen, describeWhen, useWatchers, type Watcher } from "./model";

const when = (ms: number | null) => (ms ? ` · last ${new Date(ms).toLocaleString([], { dateStyle: "short", timeStyle: "short" })}` : "");

/** Triggers of one device: on/off switch and delete (two taps) per watcher. Creating them is up to the AI and the CLI. */
export function WatcherList({ dev, open = true, mobile }: { dev: string; open?: boolean; mobile?: boolean }) {
  const { list, error, busy, toggle, remove } = useWatchers(dev, open);
  const [sure, setSure] = useState<string | null>(null);
  const btn = mobile ? "m-btn sm danger" : "btn sm danger";

  const row = (w: Watcher) => (
    <li key={w.id} className={`flex items-center gap-2 border-t border-line-soft py-2 first:border-t-0 ${w.enabled ? "" : "opacity-70"}`}>
      <div className="min-w-0 flex-1">
        <b className="block truncate font-medium">{w.name}</b>
        <small className="block truncate text-xs text-text-2" title={`${describeWhen(w.when)}, then ${describeThen(w.then)}`}>{describeWhen(w.when)} → {describeThen(w.then)}</small>
        <small className="block text-xs text-text-3">Fired {w.fired} {w.fired === 1 ? "time" : "times"}{when(w.last_fired_ms)}</small>
      </div>
      <label className={mobile ? "grid size-11 flex-none place-items-center" : "flex-none"}>
        <span className="sr-only">{w.enabled ? "Turn off" : "Turn on"} {w.name}</span>
        <Tog on={w.enabled} disabled={busy} set={() => toggle(w)} />
      </label>
      {sure === w.id
        ? <button type="button" className={btn} disabled={busy} onClick={() => { setSure(null); remove(w); }}>Delete</button>
        : <button type="button" className={`${mobile ? "m-btn sm ghost !px-2.5" : "btn sm ghost px-[7px]"}`} aria-label={`Delete ${w.name}`} onClick={() => setSure(w.id)}><Icon name="trash" /></button>}
    </li>
  );

  return (
    <div>
      {error && <p role="alert" className="mb-2 text-bad">{error}</p>}
      {list === null && !error && <p className="text-text-2" role="status">Loading</p>}
      {list?.length === 0 && <p className="text-text-2">No watchers yet. Ask the AI to watch for something, or use the ywza CLI.</p>}
      {!!list?.length && <ul aria-label="Watchers">{list.map(row)}</ul>}
    </div>
  );
}
