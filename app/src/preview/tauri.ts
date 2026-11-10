// Browser preview: stands in for @tauri-apps/api (core, event, window) so `yarn preview:ui` shows the
// UI against fake devices and chats. Only used when Vite runs with `--mode preview`.

type Handler = (e: { payload: any }) => void;
const handlers = new Map<string, Handler[]>();
const emit = (event: string, payload: unknown) =>
  setTimeout(() => handlers.get(event)?.forEach((h) => h({ payload })), 30);

export async function listen<T>(event: string, h: (e: { payload: T }) => void) {
  handlers.set(event, [...(handlers.get(event) ?? []), h as Handler]);
  return () => {};
}

export class Channel<T> {
  onmessage: (m: T) => void;
  constructor(onmessage: (m: T) => void = () => {}) { this.onmessage = onmessage; }
}

const noop = async () => {};
export const getCurrentWindow = () => ({
  minimize: noop, maximize: noop, unmaximize: noop, toggleMaximize: noop, close: noop, hide: noop,
  startDragging: noop, setFullscreen: noop, setTitle: noop,
  isMaximized: async () => false,
  isFullscreen: async () => false,
  onResized: async () => () => {},
  onFocusChanged: async () => () => {},
});

const OS = "Windows 11";
const now = Date.now();
const S1 = "7c1e0d2a-0000-4000-8000-000000000001";
const S2 = "7c1e0d2a-0000-4000-8000-000000000002";
const S3 = "7c1e0d2a-0000-4000-8000-000000000003";
const sessions: any[] = [
  { id: S1, title: "Run the tests in ywza and push the branch", status: "running", updated_ms: now - 20_000, device: "berkeley" },
  { id: S2, title: "What is Codex asking in my D:\\work terminal?", status: "done", updated_ms: now - 3_600_000, device: null },
  { id: S3, title: "Close the stale Settings window", status: "failed", updated_ms: now - 7_200_000, device: "berkeley" },
];
const events: Record<string, any[]> = {
  [S1]: [
    { type: "user", text: "Open the ywza repo in a terminal, run the Rust tests, and if they pass push the branch." },
    { type: "assistant", text: "I'll start a terminal in D:\\works\\ywza and run the tests first." },
    { type: "tool_call", id: "c1", name: "term_start", args: { command: "pwsh", cwd: "D:\\works\\ywza" } },
    { type: "tool_result", id: "c1", ok: true, text: "t4", has_image: false },
    { type: "tool_call", id: "c2", name: "term_write", args: { term: "t4", text: "cargo test --workspace" } },
    { type: "tool_result", id: "c2", ok: true, text: "sent", has_image: false },
    { type: "tool_call", id: "c3", name: "term_read", args: { term: "t4", lines: 40 } },
    { type: "tool_result", id: "c3", ok: true, text: "   Compiling ywza-agent v0.4.2\n    Finished test target(s) in 31.07s\ntest result: ok. 48 passed; 0 failed", has_image: false },
    { type: "assistant", text: "All 48 tests passed. Pushing the branch now." },
    { type: "tool_call", id: "c4", name: "term_write", args: { term: "t4", text: "git push origin toolbar-menus" } },
  ],
  [S2]: [
    { type: "user", text: "What is Codex asking in my D:\\work terminal?" },
    { type: "tool_call", id: "d1", name: "term_list", args: {} },
    { type: "tool_result", id: "d1", ok: true, text: "[t2] pwsh (Windows Terminal tab) D:\\work", has_image: false },
    { type: "tool_call", id: "d2", name: "term_read", args: { term: "t2", lines: 30 } },
    { type: "tool_result", id: "d2", ok: true, text: "Allow command `npm test`? (y/n)", has_image: false },
    { type: "done", summary: "Codex is asking whether it may run `npm test` in D:\\work. It's waiting for y/n." },
  ],
  [S3]: [
    { type: "user", text: "Close the stale Settings window." },
    { type: "tool_call", id: "m1", name: "list_windows", args: {} },
    { type: "tool_result", id: "m1", ok: true, text: "w3 Settings (notepad.exe)", has_image: false },
    { type: "tool_call", id: "m2", name: "close_window", args: { window: "w3" } },
    { type: "tool_result", id: "m2", ok: false, text: "stale: window w3 is gone", has_image: false },
    { type: "failed", error: "stale: window w3 is gone" },
  ],
};
const config: any = {
  relay_url: null,
  llm: { base_url: "http://127.0.0.1:1234/v1", api_key: "", model: "qwen3-coder-30b", max_steps: 60, vision: true, effort: null },
  ocr: { engine: "auto" },
  approval: { rules: ["close_window", "run_command", "term_write", "term_keys"].map((tool) => ({ tool, pattern: null, action: "ask" })) },
  desktop: { agent_virtual_desktop: false, idle_before_input_ms: 1000 },
  peers: [
    { id: "7f3a9c2e41d0b5a8c91e", name: "Ali's phone", view: true, chat: true, control: true, tools: false, settings: false },
    { id: "b204ee71c3a95d10f6aa", name: "berkeley", view: true, chat: false, control: false, tools: true, settings: false },
  ],
  stream: { fps: 30, bitrate_kbps: 0, cursor: true },
  log: {},
};

const rect = (x: number, y: number, width: number, height: number) => ({ x, y, width, height });
const monitors = (device: string) =>
  device === "berkeley"
    ? [
        { index: 1, name: "DELL U2723QE", x: 0, y: 0, width: 2560, height: 1440, scale: 1.5, primary: true },
        { index: 2, name: "LG 24MK430H", x: 2560, y: 0, width: 1920, height: 1080, scale: 1, primary: false },
      ]
    : [{ index: 1, name: "Built-in display", x: 0, y: 0, width: 1920, height: 1080, scale: 1.25, primary: true }];
const elements = [
  { id: 1, role: "menu_bar", name: "Application", rect: rect(220, 172, 1180, 28) },
  { id: 2, role: "menu_item", name: "File", rect: rect(228, 172, 40, 28) },
  { id: 3, role: "menu_item", name: "Edit", rect: rect(272, 172, 40, 28) },
  { id: 4, role: "edit", name: "Text editor", rect: rect(220, 204, 1160, 620) },
  { id: 5, role: "scroll_bar", name: "Vertical", rect: rect(1380, 204, 20, 620) },
  { id: 6, role: "status_bar", name: "Status", rect: rect(220, 870, 1180, 30) },
  { id: 7, role: "button", name: "Close", rect: rect(1340, 142, 60, 28) },
  { id: 8, role: "button", name: "Minimize", rect: rect(1220, 142, 60, 28) },
];

const timers = new Map<string, number[]>();
const later = (sid: string, ms: number, fn: () => void) => {
  const t = window.setTimeout(fn, ms);
  timers.set(sid, [...(timers.get(sid) ?? []), t]);
};

let nextRequest = 0;
const pending = new Map<number, { sid: string; decide: (allowed: boolean) => void }>();

/** Fake agent turn: user, assistant, a tool call on window w3, its result, a run_command that waits for approval, final reply. */
function runPrompt(out: (m: unknown) => void, msg: any) {
  const sid: string = msg.session;
  const add = (event: any) => {
    (events[sid] ??= []).push(event);
    const s = sessions.find((x) => x.id === sid);
    if (s) { s.updated_ms = Date.now(); if (event.type === "done" || event.type === "failed") s.status = event.type; }
    out({ type: "event", session: sid, event });
  };
  let s = sessions.find((x) => x.id === sid);
  if (!s) sessions.push((s = { id: sid, title: msg.text, status: "running", updated_ms: Date.now(), device: msg.device ?? null }));
  s.status = "running";
  add({ type: "user", text: msg.text });
  later(sid, 500, () => add({ type: "assistant", text: `(preview) Looking at ${msg.device ?? "this PC"}.` }));
  later(sid, 1000, () => add({ type: "tool_call", id: "p1", name: "get_elements", args: { window: "w3" } }));
  later(sid, 1900, () => add({ type: "tool_result", id: "p1", ok: true, text: "8 elements in w3 Settings (notepad.exe)", has_image: false }));
  later(sid, 2300, () => {
    const request = ++nextRequest;
    const command = "git push origin toolbar-menus";
    add({ type: "tool_call", id: "p2", name: "run_command", args: { command } });
    out({ type: "approval_request", request, session: sid, tool: "run_command", summary: `Run: ${command}`, detail: `command: ${command}\ncwd: D:\\works\\ywza` });
    pending.set(request, {
      sid,
      decide: (allowed) => {
        pending.delete(request);
        out({ type: "approval_resolved", request, allowed });
        later(sid, 400, () => {
          add({ type: "tool_result", id: "p2", ok: allowed, text: allowed ? "To origin\n   3f2a9c1..b81d0e4  toolbar-menus -> toolbar-menus" : "denied by the user", has_image: false });
          add({ type: "done", summary: "(preview) This is fake data; no agent is running." });
        });
      },
    });
  });
}

function cancel(out: (m: unknown) => void, sid: string) {
  (timers.get(sid) ?? []).forEach(clearTimeout);
  timers.delete(sid);
  pending.forEach((p, r) => { if (p.sid === sid) pending.delete(r); });
  const s = sessions.find((x) => x.id === sid);
  if (s) s.status = "failed";
  const event = { type: "failed", error: "cancelled" };
  (events[sid] ??= []).push(event);
  out({ type: "event", session: sid, event });
}

function agent(device: string, msg: any) {
  const out = (m: unknown) => emit("agent-msg", { device, msg: m });
  switch (msg.type) {
    case "list_sessions": return out({ type: "sessions", list: device === "local" ? sessions : [] });
    case "get_session": return out({ type: "session", session: msg.session, events: events[msg.session] ?? [] });
    case "get_config": return out({ type: "config", config });
    case "set_config": Object.assign(config, msg.config); return out({ type: "config", config });
    case "list_tools": return out({ type: "tools", list: ["list_windows", "get_elements", "click", "type_text", "press_keys", "scroll", "wait", "launch", "close_window", "run_command", "term_start", "term_write", "term_keys", "term_read", "screenshot", "copy"].map((name) => ({ name, description: "", parameters: {} })) });
    case "list_models": return out({ type: "models", list: ["qwen3-coder-30b", "gpt-5", "gpt-5-mini", "claude-sonnet-5"] });
    case "list_monitors": return out({ type: "monitors", list: monitors(device) });
    case "window_info":
      return out({
        type: "window_info", window: msg.window, title: "Settings - Notepad", monitor: 1,
        rect: rect(220, 140, 1180, 760), ...(msg.elements ? { elements } : {}),
      });
    case "create_pairing": return out({ type: "pairing_code", code: "482913", expires_in_secs: 600 });
    case "prompt": if (device === "local") runPrompt(out, msg); return;
    case "approve": return pending.get(msg.request)?.decide(msg.allow);
    case "cancel": return cancel(out, msg.session);
    case "live_start": return out({ type: "error", message: "Live view needs the real app (preview mode)." });
  }
}

// `?click=a|b|c` clicks the first button containing each text in turn (for screenshots).
const clicks = new URLSearchParams(location.search).get("click")?.split("|") ?? [];
clicks.forEach((text, i) => setTimeout(() => {
  const b = [...document.querySelectorAll("button")].find((el) => el.textContent?.includes(text));
  b?.click();
}, 400 + i * 300));

const devices = ["berkeley", "studio-laptop"];
const online = new Set(["berkeley"]);
const connect = (name: string) => {
  if (online.has(name)) {
    emit("connection", { device: name, connected: true, detail: "" });
    emit("agent-msg", { device: name, msg: { type: "welcome", agent_version: "0.4.2", node_id: "b204ee71c3a95d10f6aa", os: OS, session_list: [], permissions: { view: true, chat: true, control: true, tools: true, settings: false } } });
  } else emit("connection", { device: name, connected: false, detail: "no route to device (offline)" });
};

export async function invoke<T>(cmd: string, args: any = {}): Promise<T> {
  switch (cmd) {
    case "list_devices": return [...devices] as T;
    case "connection_status":
      setTimeout(() => {
        emit("agent-msg", { device: "local", msg: { type: "welcome", agent_version: "0.4.2", node_id: "e8b1c0ffee42d7a95b3c1d", os: OS, session_list: sessions, permissions: null } });
      }, 50);
      return [{ device: "local", connected: true, detail: "" }] as T;
    case "connect_device": connect(args.name); return undefined as T;
    case "pair_device":
      await new Promise((r) => setTimeout(r, 600));
      if (args.code === "000000") throw "wrong or expired code, run `ywza-agent pair` again";
      if (devices.includes(args.name)) throw `a device named \`${args.name}\` already exists`;
      devices.push(args.name);
      online.add(args.name);
      return undefined as T;
    case "remove_device":
      devices.splice(devices.indexOf(args.name), 1);
      online.delete(args.name);
      return undefined as T;
    case "send": agent(args.device ?? "local", args.msg); return undefined as T;
    default: return undefined as T;
  }
}

/** Stands in for @tauri-apps/plugin-opener. */
export async function openUrl(url: string) { window.open(url, "_blank", "noopener"); }
