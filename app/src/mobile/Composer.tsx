import { useState, type FormEvent } from "react";
import { useStore } from "../api";
import { Icon } from "../Icon";
import { stopRun, submit } from "./actions";
import { useDriver } from "./devices";
import { isActive, type Run } from "./runs";
import { toast } from "./store";

/** `run`: continue it (its driver answers); otherwise a new run on the chosen driver with `scope` as the extra devices. `onNew`: shows a button to leave `run` and start fresh. */
export function Composer({ run, scope, onSent, onNew }: { run?: Run; scope: string[]; onSent?: () => void; onNew?: () => void }) {
  const [text, setText] = useState("");
  const driver = useDriver();
  const gone = useStore((s) => !!run && !s.devices[run.driver]?.connected);
  const block = run ? (gone ? `${run.driver} is offline` : "") : driver ? "" : "No connected device has an assistant";
  const running = !!run && isActive(run);

  const go = (e: FormEvent) => {
    e.preventDefault();
    if (running) return toast("Still running. Stop it to send this"); // Enter must never stop a run
    const t = text.trim();
    if (!t || block) return;
    const err = submit(t, { run, scope });
    if (err) return toast(err);
    setText("");
    onSent?.();
  };

  return (
    <form onSubmit={go} className="flex flex-none gap-2 border-t border-line-soft px-3 pt-2 pb-3">
      {run && onNew && (
        <button type="button" className="m-btn" onClick={onNew} aria-label="New run"><Icon name="plus" /></button>
      )}
      <input value={text} onChange={(e) => setText(e.target.value)} disabled={!!block} enterKeyHint="send" autoComplete="off" aria-label="Message"
        placeholder={block || (running ? "Running. Type ahead, stop to send" : run ? "Continue this run" : "Describe a task")}
        className="h-11 min-w-0 flex-1 rounded-[10px] border border-line bg-surface px-3 outline-none placeholder:text-text-3 focus:border-accent focus:shadow-[0_0_0_3px_var(--c-accent-soft)] disabled:opacity-50" />
      {running
        ? <button type="button" className="m-btn danger" onClick={() => stopRun(run)}><Icon name="stop" />Stop</button>
        : <button type="submit" className="m-btn primary" disabled={!text.trim() || !!block}><Icon name="send" />Send</button>}
    </form>
  );
}
