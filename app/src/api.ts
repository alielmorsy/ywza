// Backend bridge + app store. Mirrors ywza-proto JSON (crates/ywza-proto/src/msg) and the Tauri
// commands in app/src-tauri/src. Every view reads state through `useStore`.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSyncExternalStore } from "react";

// ---- wire types --------------------------------------------------------------------------
export type Sid = string;
export type SessionStatus = "running" | "done" | "failed";
/** `device`: paired device the session drives with our AI; absent = this PC. `devices`: extra devices the AI may use, in the agent's own names. */
export interface Summary { id: Sid; title: string; status: SessionStatus; updated_ms: number; device?: string | null; devices?: string[] }
/** A session as the store tracks it. `driver`: the device whose AI runs it (set for sessions of paired devices; the phone UI lives on these). */
export interface Session extends Summary { driver?: string }
export type SEvent =
  | { type: "user"; text: string }
  | { type: "assistant"; text: string }
  /** `device`: the device the call runs on, in the driver's names; absent = the driver itself. */
  | { type: "tool_call"; id: string; name: string; args: any; device?: string | null }
  /** `image`: PNG (base64) shown to the user only (`show_screenshot`); the model never sees it. */
  | { type: "tool_result"; id: string; ok: boolean; text: string; has_image: boolean; image?: string | null }
  | { type: "done"; summary: string }
  | { type: "failed"; error: string }
  /** Token receipt at the end of a run; `screenshot_estimate`: tokens a screenshot-based agent would have used. Costs only when prices are set. */
  | { type: "usage"; input: number; output: number; cached: number; requests: number; tool_calls: number; screenshot_estimate: number; cost?: number | null; screenshot_cost?: number | null }
  /** The agent hit a blocker and paused until the human takes over and sends `resume`. `window`/`monitor`/`rect`: where it happened (`rect` relative to that monitor). */
  | { type: "help"; kind: HelpKind; reason: string; window?: string; title?: string; monitor?: number; rect?: Rect }
  | { type: "resumed"; digest: string };
export type HelpKind = "otp" | "uac" | "credentials" | "captcha" | "login" | "stuck";
export type Help = Extract<SEvent, { type: "help" }>;
export interface Permissions { view: boolean; chat: boolean; control: boolean; tools: boolean; settings: boolean; drive: boolean }
export type PermKey = keyof Permissions;
/** Config `[[peers]]`: permissions are flattened next to id and name. */
export interface Peer extends Permissions { id: string; name: string }
export type ApprovalAction = "allow" | "ask" | "deny";
/** First matching rule wins; `pattern` is a case-insensitive glob over a one-line summary of the call. */
export interface ApprovalRule { tool: string; pattern?: string | null; action: ApprovalAction }
export type ApprovalMode = "rules" | "auto" | "always";
export interface Config {
  peers: Peer[];
  relay_url?: string | null;
  llm: { base_url: string; api_key: string; model: string; max_steps: number; vision: boolean; effort?: Effort | null; price_in?: number | null; price_out?: number | null };
  approval: { rules: ApprovalRule[] };
  ocr: { engine: "auto" | "windows" | "paddle" };
  desktop: { agent_virtual_desktop: boolean; idle_before_input_ms: number };
  stream: { fps: number; bitrate_kbps: number; cursor: boolean };
  log: Record<string, unknown>;
}
/** `risky`: asks before each call when no approval rule matches. */
export interface ToolInfo { name: string; description: string; risky?: boolean }
export type Effort = "low" | "medium" | "high" | "xhigh" | "max";
export const EFFORTS: Effort[] = ["low", "medium", "high", "xhigh", "max"];
/** Monitor rect in virtual-desktop physical pixels. */
export interface Monitor { index: number; name: string; x: number; y: number; width: number; height: number; scale: number; primary: boolean }
export interface Rect { x: number; y: number; width: number; height: number }
/** One UI element of a window; `rect` is relative to the window's monitor, physical pixels. */
export interface Element { id: number; role: string; name: string; rect: Rect }
export interface WindowInfo { window: string; title: string; monitor: number; rect: Rect; elements?: Element[] | null }
/** Smart lens element: `id` is only valid in `lens_act` on the connection that received it. */
export interface LensElement extends Element { editable?: boolean }
/** The followed window with its tappable elements by id (rects relative to the monitor origin). */
export interface Lens { window: string; title: string; monitor: number; rect: Rect; elements: Record<number, LensElement> }
/** A watcher that fired: banner/inbox item until dismissed. `watcher` is its id (the kill switch). */
export interface Fired { watcher: string; name: string; text: string; at: number }

export type AgentMsg =
  | { type: "welcome"; agent_version: string; node_id: string; os?: string; session_list: Summary[]; permissions?: Permissions | null; llm?: boolean }
  | { type: "event"; session: Sid; event: SEvent }
  | { type: "config"; config: Config }
  | { type: "sessions"; list: Summary[] }
  | { type: "session"; session: Sid; events: SEvent[] }
  | { type: "pairing_code"; code: string; expires_in_secs: number }
  | { type: "monitors"; list: Monitor[] }
  | { type: "models"; list: string[] }
  | { type: "tools"; list: ToolInfo[] }
  | ({ type: "window_info" } & WindowInfo)
  | { type: "cursor"; png_base64?: string | null; hot_x: number; hot_y: number }
  /** Live view: the caret or focused text field, relative to the streamed monitor; null when nothing takes text. */
  | { type: "focus"; rect: Rect | null }
  | { type: "tool_result"; call: number; ok: boolean; text: string; png_base64?: string | null }
  | { type: "approval_request"; request: number; session: Sid; tool: string; summary: string; detail: string; image?: string }
  | { type: "approval_resolved"; request: number; allowed: boolean }
  | { type: "devices"; list: { name: string; node_id: string }[] }
  | { type: "image"; png_base64: string; width: number; height: number }
  | { type: "error"; message: string }
  | { type: "lens"; window: string; title: string; monitor: number; rect: Rect; full: boolean; elements: LensElement[]; removed?: number[] }
  | { type: "watch_fired"; watcher: string; name: string; text: string };
/** What a screenshot points at. */
export type Target = { type: "monitor"; id: number } | { type: "window"; id: number };
export type ClientMsg =
  /** To LOCAL only. `session` unknown to the agent = new session with that id. `device` = drive that paired device with our AI. */
  | { type: "prompt"; session: Sid; text: string; device?: string; devices?: string[]; model?: string; effort?: Effort; approval?: ApprovalMode }
  | { type: "cancel"; session: Sid }
  /** To LOCAL only. `remember` adds an allow rule for this call. */
  | { type: "approve"; request: number; allow: boolean; remember: boolean }
  | { type: "get_config" }
  | { type: "set_config"; config: Config }
  | { type: "list_sessions" }
  | { type: "get_session"; session: Sid }
  | { type: "list_models" }
  | { type: "list_tools" }
  /** The devices this agent is paired with, in its names (answered with `devices`). */
  | { type: "list_devices" }
  | { type: "screenshot"; target: Target; max_width?: number }
  | { type: "list_monitors" }
  | { type: "window_info"; window: string; elements: boolean }
  | { type: "live_start"; monitor: number }
  | { type: "live_stop" }
  | { type: "input"; event: InputEvent }
  | { type: "create_pairing" }
  /** To the agent that runs `session`, after the human fixed the blocker of a `help` event. */
  | { type: "resume"; session: Sid; note?: string }
  /** Runs one tool without an approval step; answered with `tool_result` carrying `call`. Needs `tools`. */
  | { type: "call_tool"; call: number; name: string; args?: unknown }
  /** Follows the focused window on the live view's monitor, or `window`. Send after `live_start`. Needs `view`. */
  | { type: "lens_start"; window?: string }
  | { type: "lens_stop" }
  /** Clicks a lens element; `text` types into it (replacing its content), `submit` then presses Enter. Needs `control`. */
  | { type: "lens_act"; call: number; window: string; element: number; text?: string; submit?: boolean };
/** Live view input; coordinates are 0..1 over the monitor. Key names as in `press_keys`. */
export type InputEvent =
  | { type: "mouse_move"; x: number; y: number }
  | { type: "mouse_button"; button: "left" | "right" | "middle"; down: boolean }
  | { type: "wheel"; dx: number; dy: number }
  | { type: "key"; key: string; down: boolean }
  /** Typed at the remote focus; more than one character is pasted. */
  | { type: "text"; text: string };

// ---- store -------------------------------------------------------------------------------
/** This PC's own agent. Every chat (local or remote) runs on it with our AI. */
export const LOCAL = "local";

export interface Device {
  name: string;
  connected: boolean;
  detail: string; // connection detail ("connecting", error text, ...)
  version?: string;
  nodeId?: string;
  os?: string;
  /** What this app may do on the device; null = everything (LOCAL). */
  permissions?: Permissions | null;
  config?: Config;
  monitors?: Monitor[];
  windows: Record<string, WindowInfo>;
  lastError?: string;
  /** Latest live view error, kept apart so it does not show up in chats. */
  liveError?: string;
  /** Welcome: the device has an AI of its own and can drive runs. */
  llm?: boolean;
  /** The devices this agent is paired with, by its names (`list_devices`). */
  known?: { name: string; node_id: string }[];
  /** The device's agent is older and rejected a newer message (e.g. `list_monitors`). */
  outdated?: boolean;
  /** Live view: CSS cursor with the remote cursor's shape; unset until the agent sends one. */
  cursor?: string;
  /** Live view: where typed text would land (monitor pixels), null when nothing takes text; unset until reported. */
  focus?: Rect | null;
  /** Smart lens: set while a lens runs on this connection. */
  lens?: Lens;
  /** Watchers that fired, newest first, one per watcher. */
  fired?: Fired[];
}
/** A tool call waiting for the user's decision (or already decided). */
export interface Approval { request: number; session: Sid; tool: string; summary: string; detail: string; /** Base64 PNG of the window region around the targeted element, boxed. */ image?: string; resolved?: boolean; allowed?: boolean; /** Set for approvals of paired devices' agents. */ device?: string }
/** Latest screenshot of a device (`image`), as a data URL. */
export interface Shot { src: string; w: number; h: number; at: number }
export interface State {
  devices: Record<string, Device>;
  /** All sessions, from the LOCAL agent. */
  sessions: Record<Sid, Session>;
  /** Sessions of paired devices' own agents (the phone UI's runs); the desktop UI never reads them. */
  remote: Record<Sid, Session>;
  /** Their pending approvals, keyed `device:request`. */
  remoteApprovals: Record<string, Approval>;
  shots: Record<string, Shot>;
  events: Record<Sid, SEvent[]>;
  models: string[];
  /** This PC's agent tools (`list_tools`). */
  tools: ToolInfo[];
  approvals: Record<number, Approval>;
  pairing?: { code: string; expires_at: number };
  /** Latest error from any device or command; the status bar shows it. */
  error?: { device: string; message: string; at: number };
}

let state: State = { devices: {}, sessions: {}, remote: {}, remoteApprovals: {}, shots: {}, events: {}, models: [], tools: [], approvals: {} };
const subs = new Set<() => void>();
const set = (fn: (s: State) => State) => { state = fn(state); subs.forEach((f) => f()); };
const dev = (name: string): Device =>
  state.devices[name] ?? { name, connected: false, detail: "", windows: {} };
const patch = (name: string, fn: (d: Device) => Partial<Device>) =>
  set((s) => { const d = dev(name); return { ...s, devices: { ...s.devices, [name]: { ...d, ...fn(d) } } }; });

const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };
export function useStore<T>(sel: (s: State) => T): T {
  return useSyncExternalStore(subscribe, () => sel(state));
}
/** Records an error for `device` and surfaces it in the status bar. */
export function reportError(device: string, message: string, live = false) {
  patch(device, () => (live ? { liveError: message } : { lastError: message }));
  set((s) => ({ ...s, error: { device, message, at: Date.now() } }));
}
/** Sessions this app created this run; the agent may not know them yet, so never fetch them. */
const fresh = new Set<Sid>();
/** Remote sessions whose history was asked for. */
const asked = new Set<Sid>();
export const getState = () => state;
/** Sessions that drive `device` (LOCAL = this PC), newest first. Use inside useStore with a stable result (useMemo). */
export const sessionsOf = (s: State, device: string) =>
  Object.values(s.sessions).filter((x) => (x.device ?? LOCAL) === device).sort((a, b) => b.updated_ms - a.updated_ms);

/** A device (or `device` only) with a session blocked on an unresolved approval; undefined when none. */
export const waitingOn = (s: State, device?: string): string | undefined => {
  for (const a of Object.values(s.approvals)) {
    if (a.resolved) continue;
    const d = s.sessions[a.session]?.device ?? LOCAL;
    if (!device || d === device) return d;
  }
};

// ---- commands ----------------------------------------------------------------------------
/** When each device last got a live view message; an agent error soon after belongs to the live view. */
const liveSent = new Map<string, number>();
// ponytail: errors carry no request id, so a 3 s window attributes them; add ids to the protocol if this misfiles.
const liveRecently = (device: string) => Date.now() - (liveSent.get(device) ?? 0) < 3000;

export const send = (device: string, msg: ClientMsg) => {
  if (msg.type === "live_start") patch(device, () => ({ cursor: undefined, focus: undefined, liveError: undefined })); // the new stream sends its own
  if (msg.type === "lens_start" || msg.type === "lens_stop") patch(device, () => ({ lens: undefined })); // element ids die with the lens
  const live = msg.type === "live_start" || msg.type === "live_stop" || msg.type === "input" || msg.type === "screenshot";
  if (live) liveSent.set(device, Date.now());
  return invoke("send", { device, msg }).catch((e) => reportError(device, String(e), live));
};
export interface McpClient { id: string; label: string; state: "installed" | "missing" | "unavailable" }
export const mcpStatus = () => invoke<McpClient[]>("mcp_status");
/** Adds (or removes) the ywza MCP server in that AI tool's config. */
export const mcpSet = (client: string, install: boolean) => invoke("mcp_set", { client, install });
export const connectDevice = (name: string) => invoke("connect_device", { name });
/** Pairs with another machine by the 6-digit code it shows; on success it shows up as device `name`. */
export const pairDevice = async (name: string, code: string) => {
  await invoke("pair_device", { name, code });
  patch(name, () => ({}));
  connectDevice(name).catch((e) => patch(name, () => ({ detail: String(e) })));
};
/** Tool calls and lens actions waiting for their `tool_result`, keyed `device:call`. */
const pending = new Map<string, (r: { ok: boolean; text: string }) => void>();
let callSeq = 0;
const CALL_TIMEOUT_MS = 15000;
function ask(device: string, msg: (call: number) => ClientMsg): Promise<{ ok: boolean; text: string }> {
  const call = ++callSeq, key = `${device}:${call}`;
  return new Promise((resolve) => {
    const t = setTimeout(() => { pending.delete(key); resolve({ ok: false, text: "no answer from the device" }); }, CALL_TIMEOUT_MS);
    pending.set(key, (r) => { clearTimeout(t); pending.delete(key); resolve(r); });
    send(device, msg(call));
  });
}
/** Called with the name after a device is removed (nav drops its UI state). */
export const onRemoved = new Set<(name: string) => void>();
export const removeDevice = async (name: string) => {
  await invoke("remove_device", { name });
  set((s) => {
    const { [name]: _, ...rest } = s.devices;
    return { ...s, devices: rest, remote: Object.fromEntries(Object.entries(s.remote).filter(([, x]) => x.driver !== name)) };
  });
  onRemoved.forEach((f) => f(name));
};

/**
 * Sends a prompt to an agent's AI. `device`: the paired device that AI drives on our behalf (LOCAL = none).
 * `via`: the agent that runs the session (default LOCAL); a paired device's own agent when the phone has no AI.
 * `devices`: extra devices it may use, in `via`'s names. Returns the session id (new one when `session` is null).
 */
export function prompt(device: string, text: string, session: Sid | null, opts: { model?: string; effort?: Effort; approval?: ApprovalMode; via?: string; devices?: string[] } = {}): Sid {
  const { via = LOCAL, ...rest } = opts;
  const id = session ?? crypto.randomUUID();
  // Show the new chat as running right away; the agent's first event fills it in.
  if (!session) {
    fresh.add(id);
    const x: Session = { id, title: text, status: "running", updated_ms: Date.now(), device: device === LOCAL ? null : device, devices: rest.devices };
    set((s) => via === LOCAL ? { ...s, sessions: { ...s.sessions, [id]: x } } : { ...s, remote: { ...s.remote, [id]: { ...x, driver: via } } });
  }
  send(via, { type: "prompt", session: id, text, ...rest, device: device === LOCAL ? undefined : device });
  return id;
}
export const approve = (request: number, allow: boolean, remember = false, device = LOCAL) => send(device, { type: "approve", request, allow, remember });
export const cancel = (session: Sid, device = LOCAL) => send(device, { type: "cancel", session });
export const resume = (device: string, session: Sid, note?: string) => send(device, { type: "resume", session, note: note?.trim() || undefined });
/** The help request a session waits on: its last `help` event with no later resumed/done/failed. */
export function helpOf(events: SEvent[] | undefined): Help | undefined {
  for (let i = (events?.length ?? 0) - 1; i >= 0; i--) {
    const e = events![i];
    if (e.type === "help") return e;
    if (e.type === "resumed" || e.type === "done" || e.type === "failed") return;
  }
}
export const loadSession = (session: Sid, device = LOCAL) => {
  if (fresh.has(session) || (device === LOCAL ? state.events[session] : asked.has(session))) return;
  if (device !== LOCAL) asked.add(session);
  send(device, { type: "get_session", session });
};

const setRemote = (device: string, list: Summary[]) =>
  set((s) => ({
    ...s,
    remote: { ...Object.fromEntries(Object.entries(s.remote).filter(([, x]) => x.driver !== device)), ...Object.fromEntries(list.map((x) => [x.id, { ...x, driver: device }])) },
  }));

function onMsg(device: string, m: AgentMsg) {
  switch (m.type) {
    case "welcome":
      if (device === LOCAL) set((s) => ({ ...s, sessions: Object.fromEntries(m.session_list.map((x) => [x.id, x])) }));
      else {
        setRemote(device, m.session_list);
        // Reconnected: what changed while away is unknown. Waiting approvals come again on connect
        // (and with each session), so drop ours, and reload the histories already shown.
        set((s) => ({ ...s, remoteApprovals: Object.fromEntries(Object.entries(s.remoteApprovals).filter(([, a]) => a.resolved || a.device !== device)) }));
        for (const x of m.session_list) if (state.events[x.id]) send(device, { type: "get_session", session: x.id });
      }
      return patch(device, () => ({ connected: true, version: m.agent_version, nodeId: m.node_id, os: m.os, permissions: m.permissions ?? null, llm: m.llm, lens: undefined }));
    case "event":
      // shortcut: in-app only, add tauri-plugin-notification for OS push
      return set((s) => {
        const local = device === LOCAL;
        const cur = (local ? s.sessions : s.remote)[m.session];
        // Two paired names can reach the same agent; take a session's events from its driver only.
        if (!local && cur?.driver && cur.driver !== device) return s;
        // Any event but done/failed means it runs (again): a follow-up revives a finished session.
        const status: SessionStatus = m.event.type === "done" ? "done" : m.event.type === "failed" ? "failed" : "running";
        const title = cur?.title || (m.event.type === "user" ? m.event.text : "");
        const settle = (all: Record<string, Approval>) => status === "running" ? all : Object.fromEntries(Object.entries(all).map(([k, a]) => [k, a.session === m.session && (a.device ?? LOCAL) === device && !a.resolved ? { ...a, resolved: true, allowed: false } : a]));
        const session: Session = { device: null, ...cur, id: m.session, title, status, updated_ms: Date.now(), ...(!local && { driver: device }) };
        const events = { ...s.events, [m.session]: [...(s.events[m.session] ?? []), m.event] };
        return local
          ? { ...s, events, approvals: settle(s.approvals as Record<string, Approval>), sessions: { ...s.sessions, [m.session]: session } }
          : { ...s, events, remoteApprovals: settle(s.remoteApprovals), remote: { ...s.remote, [m.session]: session } };
      });
    case "sessions":
      if (device === LOCAL) set((s) => ({ ...s, sessions: Object.fromEntries(m.list.map((x) => [x.id, x])) }));
      else setRemote(device, m.list);
      return;
    case "session": asked.add(m.session); return set((s) => ({ ...s, events: { ...s.events, [m.session]: m.events } }));
    case "config": return patch(device, () => ({ config: m.config }));
    case "pairing_code": return set((s) => ({ ...s, pairing: { code: m.code, expires_at: Date.now() + m.expires_in_secs * 1000 } }));
    case "monitors": return patch(device, () => ({ monitors: m.list }));
    case "models": return set((s) => ({ ...s, models: m.list }));
    case "tools": if (device === LOCAL) set((s) => ({ ...s, tools: m.list })); return;
    case "window_info": {
      const { type: _, ...w } = m;
      return patch(device, (d) => ({ windows: { ...d.windows, [w.window]: w } }));
    }
    case "cursor": return patch(device, () => ({
      cursor: m.png_base64 ? `url(data:image/png;base64,${m.png_base64}) ${m.hot_x} ${m.hot_y}, auto` : "none",
    }));
    case "focus": return patch(device, () => ({ focus: m.rect }));
    case "approval_request": {
      const a: Approval = { request: m.request, session: m.session, tool: m.tool, summary: m.summary, detail: m.detail, image: m.image };
      if (device === LOCAL) return set((s) => ({ ...s, approvals: { ...s.approvals, [m.request]: a } }));
      const driver = state.remote[m.session]?.driver;
      if (driver && driver !== device) return; // same agent under another name, see "event"
      return set((s) => ({ ...s, remoteApprovals: { ...s.remoteApprovals, [`${device}:${m.request}`]: { ...a, device } } }));
    }
    case "approval_resolved": {
      const key = `${device}:${m.request}`;
      const a = device === LOCAL ? state.approvals[m.request] : state.remoteApprovals[key];
      if (!a) return;
      const done = { ...a, resolved: true, allowed: m.allowed };
      return set((s) => device === LOCAL ? { ...s, approvals: { ...s.approvals, [m.request]: done } } : { ...s, remoteApprovals: { ...s.remoteApprovals, [key]: done } });
    }
    case "devices": return patch(device, () => ({ known: m.list }));
    case "image": return set((s) => ({ ...s, shots: { ...s.shots, [device]: { src: `data:image/png;base64,${m.png_base64}`, w: m.width, h: m.height, at: Date.now() } } }));
    case "tool_result": {
      const done = pending.get(`${device}:${m.call}`);
      if (done) done({ ok: m.ok, text: m.text });
      return;
    }
    case "lens":
      return patch(device, (d) => {
        const keep = m.full || d.lens?.window !== m.window ? {} : d.lens!.elements;
        const elements: Record<number, LensElement> = { ...keep, ...Object.fromEntries(m.elements.map((e) => [e.id, e])) };
        for (const id of m.removed ?? []) delete elements[id];
        return { lens: { window: m.window, title: m.title, monitor: m.monitor, rect: m.rect, elements } };
      });
    case "watch_fired":
      return patch(device, (d) => ({ fired: [{ watcher: m.watcher, name: m.name, text: m.text, at: Date.now() }, ...(d.fired ?? []).filter((f) => f.watcher !== m.watcher)].slice(0, 20) }));
    case "error": {
      const unknown = /unknown variant `(\w+)`/.exec(m.message)?.[1];
      // No device list coming back: drive alone instead of waiting forever.
      if (unknown === "list_devices" || m.message === "permission denied: drive") patch(device, () => ({ known: [] }));
      if (m.message === "permission denied: drive") return reportError(device, "does not let this phone use its other devices. Turn on Drive other devices for it in Access.");
      if (!unknown) return reportError(device, m.message, liveRecently(device));
      const first = !state.devices[device]?.outdated;
      // An older agent streams monitor 0 only and knows nothing newer; degrade instead of failing.
      patch(device, (d) => ({
        outdated: true,
        ...(unknown === "list_monitors" && !d.monitors && { monitors: [{ index: 0, name: "Main monitor", x: 0, y: 0, width: 16, height: 9, scale: 1, primary: true }] }),
      }));
      if (first) reportError(device, "runs an older ywza agent. Update it to pick monitors, follow windows and more.");
      return;
    }
  }
}

interface Status { device: string; connected: boolean; detail: string }
const onStatus = (s: Status) => patch(s.device, () => ({ connected: s.connected, detail: s.detail }));

/** Call once at startup: subscribes to backend events and connects every paired device. */
export async function boot() {
  await listen<{ device: string; msg: AgentMsg }>("agent-msg", (e) => onMsg(e.payload.device, e.payload.msg));
  await listen<Status>("connection", (e) => onStatus(e.payload));
  patch(LOCAL, () => ({}));
  (await invoke<Status[]>("connection_status")).forEach(onStatus);
  const names = await invoke<string[]>("list_devices");
  names.forEach((n) => { patch(n, () => ({})); connectDevice(n).catch((e) => patch(n, () => ({ detail: String(e) }))); });
}

/** Runs an agent tool directly (needs the `tools` permission). Never rejects: failures come back as `ok: false`. */
export const callTool = (device: string, name: string, args: unknown = {}) => ask(device, (call) => ({ type: "call_tool", call, name, args }));
/** Clicks a lens element, or types `text` into it (and presses Enter with `submit`). Needs `control`. */
export const lensAct = (device: string, window: string, element: number, o: { text?: string; submit?: boolean } = {}) =>
  ask(device, (call) => ({ type: "lens_act", call, window, element, ...o }));
/** Hides a fired-watcher notice. */
export const dismissFired = (device: string, watcher: string) =>
  patch(device, (d) => ({ fired: (d.fired ?? []).filter((f) => f.watcher !== watcher) }));
