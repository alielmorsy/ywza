import { useEffect, useLayoutEffect, useState, type RefObject } from "react";
import { Icon } from "../../Icon";
import { EFFORTS, LOCAL, cancel, prompt, send, useStore, type ApprovalMode, type Effort } from "../../api";
import { setComposer, setUi, useNav } from "../../nav";
import { label } from "./util";

interface Props {
  dev: string; session: string | null; running: boolean; compact?: boolean;
  text: string; setText: (t: string) => void; inputRef: RefObject<HTMLTextAreaElement | null>;
}

const MAX_H = 120;

export function Composer({ dev, session, running, compact, text, setText, inputRef }: Props) {
  const model = useNav((n) => n.model);
  const effort = useNav((n) => n.effort);
  const approval = useNav((n) => n.approval);
  const models = useStore((s) => s.models);
  const localUp = useStore((s) => !!s.devices[LOCAL]?.connected);
  const localModel = useStore((s) => s.devices[LOCAL]?.config?.llm.model);
  const target = useStore((s) => s.devices[dev]);
  const lastError = useStore((s) => s.devices[dev]?.lastError);
  const localError = useStore((s) => s.devices[LOCAL]?.lastError);
  const [hint, setHint] = useState(false);
  const STOP_FIRST = "Stop the current run to send another message";

  useEffect(() => {
    if (!localUp) return;
    send(LOCAL, { type: "list_models" });
    if (!localModel) send(LOCAL, { type: "get_config" });
  }, [localUp]); // eslint-disable-line react-hooks/exhaustive-deps

  useLayoutEffect(() => {
    const el = inputRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, MAX_H) + "px";
  }, [text, inputRef]);

  const blocked = !localUp ? "This PC's agent is not running"
    : dev === LOCAL ? ""
    : !target?.connected ? `${dev} is offline`
    : target.permissions?.tools === false ? `${dev} does not let this PC run tools` : "";

  const errors = [...new Set([lastError, dev === LOCAL ? undefined : localError].filter((m): m is string => !!m))];

  const submit = () => {
    const t = text.trim();
    if (!t || blocked) return;
    if (running) { setHint(true); return; }
    const id = prompt(dev, t, session, { model, effort, approval: approval ?? "rules" }); // always set: the agent gates app chats only
    if (id !== session) setUi(dev, { session: id });
    setText("");
  };

  const modelOpts = model && !models.includes(model) ? [model, ...models] : models;
  const sel = "field h-[26px] w-auto max-w-[190px] min-w-0 truncate text-xs";

  return (
    <div className={compact ? "px-2.5 pt-1.5 pb-2.5" : "mx-auto w-full max-w-[768px] px-6 pt-2 pb-4"}>
      {(blocked || errors.length > 0) && (
        <div className="mb-1.5 flex flex-col gap-0.5 text-xs text-text-2" role="status">
          {blocked && <div className="flex items-center gap-1.5"><Icon name="warn" className="size-3.5" />{blocked}</div>}
          {errors.map((m) => <div key={m} className="break-words select-text">{m}</div>)}
        </div>
      )}
      {hint && running && <div className="mb-1.5 text-xs text-text-2" role="status">{STOP_FIRST}</div>}
      <div className="flex items-end gap-1.5 rounded-md border border-line bg-surface py-1.5 pr-1.5 pl-3 transition-colors focus-within:border-accent focus-within:shadow-[0_0_0_3px_var(--c-accent-soft)]">
        <textarea ref={inputRef} rows={1} value={text} aria-label="Message" placeholder={`Ask ${label(dev)} to do something`}
          className="min-h-[22px] flex-1 resize-none border-0 bg-transparent py-1 leading-[1.45] outline-none placeholder:text-text-3"
          onChange={(e) => { setText(e.target.value); setHint(false); }}
          onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); submit(); } }} />
        {running && session
          ? <button className="btn size-[30px] flex-none justify-center rounded-md p-0" title="Stop the agent" aria-label="Stop"
              onClick={() => cancel(session)}><Icon name="stop" /></button>
          : <button className="btn primary size-[30px] flex-none justify-center rounded-md p-0 aria-disabled:cursor-not-allowed aria-disabled:opacity-40"
              title={blocked || "Send (Enter)"} aria-label="Send" aria-disabled={!!blocked || !text.trim()} onClick={submit}><Icon name="send" /></button>}
      </div>
      <div className="mt-1.5 flex flex-col gap-1.5 px-0.5 text-xs text-text-3">
        <div className="flex items-center gap-3">
          <label className="flex min-w-0 items-center gap-1.5">Model
            <select className={sel} value={model ?? ""} onChange={(e) => setComposer({ model: e.target.value || undefined })}>
              <option value="">Default ({localModel || "agent default"})</option>
              {modelOpts.map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </label>
          <label className="flex items-center gap-1.5">Effort
            <select className={sel} value={effort ?? ""} onChange={(e) => setComposer({ effort: (e.target.value || undefined) as Effort | undefined })}>
              <option value="">Default</option>
              {EFFORTS.map((x) => <option key={x} value={x}>{x}</option>)}
            </select>
          </label>
        </div>
        <label className="flex min-w-0 items-center gap-1.5">Approvals
          <select className={sel} value={approval ?? "rules"} onChange={(e) => setComposer({ approval: e.target.value === "rules" ? undefined : (e.target.value as ApprovalMode) })}>
            <option value="rules">Follow rules</option>
            <option value="auto">Auto: run without asking</option>
            <option value="always">Ask before every action</option>
          </select>
        </label>
      </div>
    </div>
  );
}
