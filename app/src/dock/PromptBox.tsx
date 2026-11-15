import { useEffect, useRef, useState } from "react";
import { Icon } from "../Icon";
import { LOCAL, useStore, type ContextItem, type DockOpen } from "../api";
import { ApprovalSelect } from "./approval";
import { actionsFor, contextText } from "./context";

const MAX_CHIPS = 5;

const chip = (i: ContextItem) =>
  i.kind === "image" ? "Image"
  : i.kind === "path" ? i.path.split(/[\\/]/).filter(Boolean).pop() ?? i.path
  : i.text.split("\n").map((l) => l.trim()).find(Boolean) ?? "Text";

/** `onHeight`: natural height in CSS px while context is attached, so the window can grow to fit. */
export function PromptBox({ context, onSubmit, onHeight }: { context?: DockOpen; onSubmit: (text: string) => void; onHeight: (h: number) => void }) {
  const [text, setText] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const box = useRef<HTMLDivElement>(null);
  const up = useStore((s) => !!s.devices[LOCAL]?.connected);

  // The window gets focus after the box mounts.
  useEffect(() => {
    const focus = () => input.current?.focus();
    focus();
    addEventListener("focus", focus);
    return () => removeEventListener("focus", focus);
  }, []);

  useEffect(() => {
    if (!context) return;
    // +2 for the shell's border.
    const ro = new ResizeObserver(() => onHeight(Math.ceil(box.current?.offsetHeight ?? 0) + 2));
    ro.observe(box.current!);
    return () => ro.disconnect();
  }, [context, onHeight]);

  // Clipboard content is quoted as data; nothing runs until the user clicks or presses Enter.
  const send = (t: string) => { if (t && up) onSubmit(context ? `${contextText(context)}\n${t}` : t); };
  const submit = () => send(text.trim());

  const row = (
    <div className={`flex items-center gap-2 pr-3 pl-5 ${context ? "h-[56px]" : "h-full"}`}>
      <input ref={input} value={text} aria-label="Ask ywza" spellCheck={false} autoComplete="off"
        placeholder={!up ? "This PC's agent is not running" : context ? "Or say what to do with this..." : "Ask ywza to do something on this PC..."}
        className="min-w-0 flex-1 border-0 bg-transparent text-[15px] outline-none placeholder:text-text-3"
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter" && !e.nativeEvent.isComposing) { e.preventDefault(); submit(); } }} />
      <ApprovalSelect />
      <button type="button" className="btn primary size-[34px] flex-none justify-center rounded-md p-0 aria-disabled:cursor-not-allowed aria-disabled:opacity-40"
        title="Send (Enter)" aria-label="Send" aria-disabled={!up || !text.trim()} onClick={submit}><Icon name="send" /></button>
    </div>
  );
  if (!context) return row;

  const more = context.items.length - MAX_CHIPS;
  const actions = actionsFor(context);
  return (
    <div ref={box} className="flex flex-col">
      <ul aria-label="Attached" className="flex min-w-0 gap-1.5 overflow-hidden px-5 pt-3">
        {context.items.slice(0, MAX_CHIPS).map((i, n) => (
          <li key={n} title={i.kind === "text" ? i.text.slice(0, 300) : i.path}
            className="flex h-[22px] min-w-0 max-w-[200px] items-center rounded-md border border-line-soft bg-surface px-2 text-xs text-text-2">
            <span className="truncate">{chip(i)}</span>
          </li>
        ))}
        {more > 0 && <li className="flex h-[22px] flex-none items-center px-1 text-xs text-text-3">+{more} more</li>}
      </ul>
      {row}
      {actions.length > 0 && (
        <div className="flex flex-wrap gap-1.5 px-5 pb-3">
          {actions.map((a) => (
            <button key={a.label} type="button" className="btn sm" aria-disabled={!up} onClick={() => send(a.prompt)}>{a.label}</button>
          ))}
        </div>
      )}
    </div>
  );
}
