import { useEffect, useRef, useState } from "react";
import { Icon } from "../Icon";
import { LOCAL, useStore } from "../api";

export function PromptBox({ onSubmit }: { onSubmit: (text: string) => void }) {
  const [text, setText] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const up = useStore((s) => !!s.devices[LOCAL]?.connected);

  // The window gets focus after the box mounts.
  useEffect(() => {
    const focus = () => input.current?.focus();
    focus();
    addEventListener("focus", focus);
    return () => removeEventListener("focus", focus);
  }, []);

  const submit = () => { const t = text.trim(); if (t && up) onSubmit(t); };

  return (
    <div className="flex h-full items-center gap-2 pr-3 pl-5">
      <input ref={input} value={text} aria-label="Ask ywza" spellCheck={false} autoComplete="off"
        placeholder={up ? "Ask ywza to do something on this PC..." : "This PC's agent is not running"}
        className="min-w-0 flex-1 border-0 bg-transparent text-[15px] outline-none placeholder:text-text-3"
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter" && !e.nativeEvent.isComposing) { e.preventDefault(); submit(); } }} />
      <button type="button" className="btn primary size-[34px] flex-none justify-center rounded-md p-0 aria-disabled:cursor-not-allowed aria-disabled:opacity-40"
        title="Send (Enter)" aria-label="Send" aria-disabled={!up || !text.trim()} onClick={submit}><Icon name="send" /></button>
    </div>
  );
}
