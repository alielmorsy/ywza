import { useEffect, useMemo, useRef, useState } from "react";
import { EFFORTS, LOCAL, send, useStore, type Config, type Effort } from "../api";
import { Icon } from "../Icon";
import { ApprovalRules } from "./ApprovalRules";
import { Num, Row, Section, Tog } from "./Form";
import { cleanConfig, saveConfig, trimRules } from "./localConfig";

const OTHER = "\0other";

/** Why the draft can't be saved, or null. */
function problem(c: Config): string | null {
  try {
    if (!/^https?:$/.test(new URL(c.llm.base_url.trim()).protocol)) return "Endpoint must be an http or https URL";
  } catch {
    return "Endpoint must be an http or https URL";
  }
  if (!c.llm.model.trim()) return "Pick a default model";
  if (c.llm.max_steps < 1) return "Max steps must be at least 1";
  return null;
}

function ModelField({ value, models, onChange }: { value: string; models: string[]; onChange: (v: string) => void }) {
  const [custom, setCustom] = useState(false);
  const free = custom || models.length === 0 || !models.includes(value);
  return (
    <div className="flex w-full flex-col gap-1.5">
      {models.length > 0 && (
        <select className="field" value={free ? OTHER : value} onChange={(e) => (e.target.value === OTHER ? setCustom(true) : (setCustom(false), onChange(e.target.value)))}>
          {models.map((m) => <option key={m} value={m}>{m}</option>)}
          <option value={OTHER}>Other</option>
        </select>
      )}
      {free && <input className="field mono" value={value} placeholder="Model name" spellCheck={false} onChange={(e) => onChange(e.target.value)} />}
    </div>
  );
}

export default function Settings() {
  const local = useStore((s) => s.devices[LOCAL]);
  const models = useStore((s) => s.models);
  const connected = !!local?.connected;
  const base = useMemo(() => (local?.config ? cleanConfig(local.config) : undefined), [local?.config]);
  const [draft, setDraft] = useState<Config>();
  const prev = useRef<Config | undefined>(undefined);
  const [eye, setEye] = useState(false);

  useEffect(() => {
    if (!connected) return;
    send(LOCAL, { type: "get_config" });
    send(LOCAL, { type: "list_models" });
  }, [connected]);

  useEffect(() => {
    if (!base) return;
    setDraft((d) => (!d || JSON.stringify(d) === JSON.stringify(prev.current) ? structuredClone(base) : d));
    prev.current = base;
  }, [base]);

  if (!connected) return <p className="p-6 text-text-3">This PC's agent is not running{local?.detail ? `: ${local.detail}` : ""}. Settings load once it starts.</p>;
  if (!draft || !base) return <p className="p-6 text-text-3">Loading settings...</p>;

  const dirty = JSON.stringify(draft) !== JSON.stringify(base);
  const up = (fn: (c: Config) => void) => setDraft((d) => { const c = structuredClone(d!); fn(c); return c; });
  const invalid = problem(draft);
  const save = () => {
    if (!dirty || invalid) return;
    // Latest peers, so a save from an older page does not undo access edits made meanwhile.
    const out = { ...draft, peers: local!.config!.peers, approval: { rules: trimRules(draft.approval.rules) } };
    saveConfig(out, "Settings saved");
    setDraft({ ...out, llm: { ...out.llm, api_key: "" } });
  };

  return (
    <div className="flex h-full flex-col">
      <header className="flex h-10 flex-none items-center gap-3 border-b border-line-soft px-4">
        <h1 className="text-sm font-semibold">Settings</h1>
        <span className="text-xs text-text-2">For this PC. Every chat, on any device, uses this AI.</span>
      </header>

      <div className="min-h-0 flex-1 overflow-auto">
        <div className="mx-auto flex max-w-[700px] flex-col gap-5 px-6 py-5">
          <Section h="AI">
            <Row t="Endpoint" d="OpenAI-compatible base URL">
              <input className="field mono" value={draft.llm.base_url} spellCheck={false} aria-invalid={invalid?.startsWith("Endpoint")} onChange={(e) => up((c) => (c.llm.base_url = e.target.value))} />
            </Row>
            <Row t="API key" d="Leave empty to keep the saved key">
              <input className="field mono" type={eye ? "text" : "password"} value={draft.llm.api_key} placeholder="Saved key is kept" autoComplete="off" spellCheck={false}
                onChange={(e) => up((c) => (c.llm.api_key = e.target.value))} />
              <button type="button" className={`btn ghost sm ${eye ? "!bg-surface-3 !text-text" : ""}`} aria-label="Show key" title={eye ? "Hide key" : "Show key"} aria-pressed={eye} onClick={() => setEye(!eye)}><Icon name="eye" /></button>
            </Row>
            <Row t="Default model" d="Chats use it unless you pick another" w="w-[320px]">
              <ModelField value={draft.llm.model} models={models} onChange={(v) => up((c) => (c.llm.model = v))} />
            </Row>
            <Row t="Default effort" d="How hard the model thinks" w="w-[150px]">
              <select className="field" value={draft.llm.effort ?? ""} onChange={(e) => up((c) => (c.llm.effort = (e.target.value || null) as Effort | null))}>
                <option value="">Default</option>
                {EFFORTS.map((x) => <option key={x} value={x}>{x}</option>)}
              </select>
            </Row>
            <Row t="Max steps" d="Tool calls allowed per prompt" w="w-[150px]">
              <Num value={draft.llm.max_steps} min={1} max={1000} onChange={(v) => up((c) => (c.llm.max_steps = v))} />
            </Row>
            <Row t="Model understands screenshots" d="Off hides the screenshot tool from the model" w="w-auto">
              <Tog on={draft.llm.vision} set={(v) => up((c) => (c.llm.vision = v))} />
            </Row>
          </Section>

          <Section h="Approvals">
            <ApprovalRules rules={draft.approval.rules} up={(fn) => up((c) => fn(c.approval.rules))} />
          </Section>

          <Section h="Desktop">
            <Row t="Idle before input" d="Milliseconds without your input before the agent clicks or types" w="w-[150px]">
              <Num value={draft.desktop.idle_before_input_ms} onChange={(v) => up((c) => (c.desktop.idle_before_input_ms = v))} />
            </Row>
            <Row t="Text recognition" w="w-[150px]">
              <select className="field" value={draft.ocr.engine} onChange={(e) => up((c) => (c.ocr.engine = e.target.value as Config["ocr"]["engine"]))}>
                <option value="auto">Automatic</option>
                <option value="windows">Windows</option>
                <option value="paddle">PaddleOCR</option>
              </select>
            </Row>
          </Section>

          <Section h="Screen sharing">
            <Row t="Frame rate" d="Frames per second" w="w-[150px]">
              <Num value={draft.stream.fps} min={1} max={120} onChange={(v) => up((c) => (c.stream.fps = v))} />
            </Row>
            <Row t="Bitrate" d="kbit/s, 0 adapts to the connection" w="w-[150px]">
              <Num value={draft.stream.bitrate_kbps} onChange={(v) => up((c) => (c.stream.bitrate_kbps = v))} />
            </Row>
            <Row t="Show mouse cursor" w="w-auto">
              <Tog on={draft.stream.cursor} set={(v) => up((c) => (c.stream.cursor = v))} />
            </Row>
          </Section>

          <Section h="Network">
            <Row t="Relay URL" d="Empty uses the public relays. Applies after the agent restarts">
              <input className="field mono" value={draft.relay_url ?? ""} placeholder="Public relays" spellCheck={false}
                onChange={(e) => up((c) => (c.relay_url = e.target.value.trim() || null))} />
            </Row>
          </Section>
        </div>
      </div>

      <div className="flex h-10 flex-none items-center gap-2 border-t border-line-soft bg-chrome px-4">
        <span className={`flex-1 text-xs ${invalid ? "text-bad" : "text-text-2"}`} aria-live="polite">{invalid ?? (dirty ? "Unsaved changes" : "All changes saved")}</span>
        <button className="btn sm" aria-disabled={!dirty} title={dirty ? undefined : "Nothing to revert"} onClick={() => { if (dirty) setDraft(structuredClone(base)); }}>Revert</button>
        <button className="btn sm primary" aria-disabled={!dirty || !!invalid} title={invalid ?? (dirty ? undefined : "No changes to save")} onClick={save}>Save</button>
      </div>
    </div>
  );
}
