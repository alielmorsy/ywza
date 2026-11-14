import { useEffect, useState } from "react";
import { LOCAL, getState, prompt, useStore } from "../api";
import { pushFile } from "../xfer";
import { canDrive, devColor, pickDriver } from "./devices";
import { getM, openRun, toast, useLabel } from "./store";

interface Shared { text?: string; subject?: string; files?: string[] }

const DEST = "Downloads/";
const base = (p: string) => p.split(/[\/]/).pop() ?? p;

/** The share the Android activity kept (and clears it). */
function take(): Shared | null {
  try {
    const j = (window as unknown as { ywzaShare?: { take(): string | null } }).ywzaShare?.take();
    return j ? JSON.parse(j) : null;
  } catch { return null; }
}

/** Opens when something is shared to the app from the system share sheet: pick a PC, send the files, start a task there. */
export function ShareSheet() {
  const L = useLabel();
  const [share, setShare] = useState<Shared | null>(null);
  const [dev, setDev] = useState<string>();
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const devices = useStore((s) => Object.values(s.devices).filter((d) => d.name !== LOCAL && canDrive(d) && d.permissions?.chat !== false).map((d) => d.name).sort().join("\n"));
  const pcs = devices ? devices.split("\n") : [];
  const files = share?.files ?? [];

  useEffect(() => {
    const open = () => {
      const s = take();
      if (!s) return;
      setShare(s);
      setNote("");
      setDev(pickDriver(getState(), getM().driver));
    };
    open();
    addEventListener("ywza-share", open);
    return () => removeEventListener("ywza-share", open);
  }, []);

  if (!share) return null;
  const target = dev && pcs.includes(dev) ? dev : pcs[0];

  const send = async () => {
    if (!target || busy) return;
    setBusy(true);
    try {
      for (const f of files) {
        const r = await pushFile(target, f, DEST);
        if (r.includes("\n")) throw r;
      }
      const lines = [note.trim(), share.subject?.trim(), share.text?.trim()].filter(Boolean) as string[];
      if (files.length) lines.push(`The user shared ${files.map(base).join(", ")}, saved at ${files.map((f) => `~/${DEST}${base(f)}`).join(", ")} on this PC.`);
      const id = prompt(LOCAL, lines.join("\n\n"), null, { via: target, approval: "rules" });
      setShare(null);
      openRun(id);
      toast(`Run started on ${L(target)}`);
    } catch (e) {
      toast(`Could not send: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="absolute inset-0 z-50 flex items-end bg-backdrop/70" onClick={() => !busy && setShare(null)}>
      <div role="dialog" aria-modal="true" aria-label="Send to a PC" onClick={(e) => e.stopPropagation()}
        className="m-rise flex max-h-full w-full flex-col gap-3 overflow-y-auto rounded-t-2xl border-t border-line bg-ground px-5 pt-5 pb-[calc(env(safe-area-inset-bottom)+16px)]">
        <h2 className="text-base">Send to a PC</h2>
        <div className="flex flex-col gap-1 rounded-[10px] border border-line bg-chrome px-3 py-2 text-[13px]">
          {share.subject && <b className="truncate">{share.subject}</b>}
          {share.text && <p className="line-clamp-4 break-words text-text-2">{share.text}</p>}
          {files.map((f) => <span key={f} className="mono truncate text-[12px]">{base(f)}</span>)}
        </div>
        {pcs.length ? (
          <div className="flex flex-wrap gap-2" role="radiogroup" aria-label="PC">
            {pcs.map((n) => (
              <button key={n} type="button" role="radio" aria-checked={n === target} onClick={() => setDev(n)}
                className={`m-btn ${n === target ? "primary" : ""}`}>
                <i className="size-2 rounded-full" style={{ background: devColor(n) }} />{L(n)}
              </button>
            ))}
          </div>
        ) : <p className="text-xs text-warn" role="status">No connected PC has an assistant</p>}
        <input value={note} onChange={(e) => setNote(e.target.value)} autoComplete="off" aria-label="Note" placeholder="Add a note (optional)"
          className="h-12 rounded-[10px] border border-line bg-chrome px-3 outline-none focus:border-accent focus:shadow-[0_0_0_3px_var(--c-accent-soft)]" />
        <div className="flex gap-2">
          <button className="m-btn flex-1" disabled={busy} onClick={() => setShare(null)}>Cancel</button>
          <button className="m-btn primary flex-1" disabled={!target || busy} onClick={send}>{busy ? "Sending" : "Send"}</button>
        </div>
      </div>
    </div>
  );
}
