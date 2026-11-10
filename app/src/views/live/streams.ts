// One live stream per device (the agent allows only one). Every visible view adds a sink; the stream
// runs while at least one sink is present and the last monitor request wins.
import { Channel, invoke } from "@tauri-apps/api/core";
import { getState, reportError, send } from "../../api";
import { createDecoder } from "./decode";

export type Sink = (f: VideoFrame) => void;
export interface StreamInfo { monitor: number; live: boolean }

interface Stream {
  info: StreamInfo;
  sinks: Set<Sink>;
  subs: Set<() => void>;
  gen: number;
  started: boolean;
  stopDecoding?: () => void;
}

const streams = new Map<string, Stream>();

function of(dev: string): Stream {
  let s = streams.get(dev);
  if (!s) streams.set(dev, s = { info: { monitor: 0, live: false }, sinks: new Set(), subs: new Set(), gen: 0, started: false });
  return s;
}

function update(s: Stream, p: Partial<StreamInfo>) {
  const n = { ...s.info, ...p };
  if (n.monitor === s.info.monitor && n.live === s.info.live) return;
  s.info = n;
  s.subs.forEach((f) => f());
}

async function start(dev: string, s: Stream) {
  const gen = ++s.gen;
  const dec = createDecoder((f) => { s.sinks.forEach((k) => k(f)); update(s, { live: true }); });
  const ch = new Channel<ArrayBuffer>();
  ch.onmessage = (b) => { try { dec.push(b); } catch (e) { console.warn("live frame:", e); } };
  s.stopDecoding = () => { ch.onmessage = () => {}; dec.close(); };
  try { await invoke("live_channel", { device: dev, channel: ch }); } catch (e) {
    reportError(dev, String(e));
    if (s.gen === gen) { s.stopDecoding?.(); s.stopDecoding = undefined; }
    return;
  }
  if (s.gen !== gen) return; // released while the channel was being set up
  s.started = true;
  send(dev, { type: "live_start", monitor: s.info.monitor });
}

function stop(dev: string, s: Stream) {
  s.gen++;
  s.stopDecoding?.();
  s.stopDecoding = undefined;
  if (s.started && getState().devices[dev]?.connected) send(dev, { type: "live_stop" });
  s.started = false;
  update(s, { live: false });
}

/** Adds `sink` and starts the stream if it is the first one. Returns the release function. */
export function acquire(dev: string, sink: Sink): () => void {
  const s = of(dev);
  s.sinks.add(sink);
  if (s.sinks.size === 1) start(dev, s);
  return () => {
    s.sinks.delete(sink);
    if (!s.sinks.size) stop(dev, s);
  };
}

export function setMonitor(dev: string, monitor: number) {
  const s = of(dev);
  if (s.info.monitor === monitor) return;
  update(s, { monitor });
  if (s.started) send(dev, { type: "live_start", monitor });
}

export const infoOf = (dev: string) => of(dev).info;
export const subscribe = (dev: string, f: () => void) => {
  const s = of(dev);
  s.subs.add(f);
  return () => { s.subs.delete(f); };
};
