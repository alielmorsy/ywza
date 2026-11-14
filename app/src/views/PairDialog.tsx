import { useEffect, useMemo, useState, type FormEvent } from "react";
import qrcode from "qrcode-generator";
import { LOCAL, getState, pairDevice, send, useStore } from "../api";
import { flash, select } from "../nav";
import { Icon } from "../Icon";
import { Dialog } from "./Dialog";
import { pairUri } from "./pairCode";

const group = (c: string) => c.replace(/(\d{3})(?=\d)/, "$1 ");
const clock = (secs: number) => `${String(Math.floor(secs / 60)).padStart(2, "0")}:${String(secs % 60).padStart(2, "0")}`;

function Qr({ text }: { text: string }) {
  const { path, size } = useMemo(() => {
    const q = qrcode(0, "M");
    q.addData(text);
    q.make();
    const n = q.getModuleCount();
    let d = "";
    for (let r = 0; r < n; r++) for (let c = 0; c < n; c++) if (q.isDark(r, c)) d += `M${c} ${r}h1v1h-1z`;
    return { path: d, size: n };
  }, [text]);
  return (
    <svg viewBox={`-3 -3 ${size + 6} ${size + 6}`} className="size-[140px] rounded-md bg-text text-ink" role="img" aria-label="QR code of the pairing code" shapeRendering="crispEdges">
      <path d={path} fill="currentColor" />
    </svg>
  );
}

/** Backend pairing errors speak to CLI users ("run `ywza-agent pair` again"); say what to do here instead. */
const pairError = (e: string) =>
  /no agent answered|could not reach/i.test(e) ? "Could not reach the other device. Check the code, or make a new one there and try again."
  : /wrong or expired code|code expired/i.test(e) ? "Wrong or expired code. Make a new one on the other device and try again."
  : e;

export function LetIn() {
  const connected = useStore((s) => !!s.devices[LOCAL]?.connected);
  const detail = useStore((s) => s.devices[LOCAL]?.detail);
  const pairing = useStore((s) => s.pairing);
  const error = useStore((s) => s.error);
  const [stale, setStale] = useState(() => getState().pairing?.expires_at);
  const [since, setSince] = useState(Date.now());
  const [now, setNow] = useState(Date.now());
  const [copied, setCopied] = useState(false);

  const request = () => { setStale(getState().pairing?.expires_at); setSince(Date.now()); send(LOCAL, { type: "create_pairing" }); };
  useEffect(() => { if (connected) request(); }, [connected]);
  useEffect(() => { const t = setInterval(() => setNow(Date.now()), 1000); return () => clearInterval(t); }, []);

  const fresh = pairing && pairing.expires_at !== stale ? pairing : undefined;
  const left = fresh ? Math.max(0, Math.ceil((fresh.expires_at - now) / 1000)) : 0;
  const expired = !!fresh && left === 0;
  const failed = !fresh && error?.device === LOCAL && error.at >= since ? error.message : "";
  const copy = () => {
    if (!fresh || expired) return;
    navigator.clipboard.writeText(fresh.code).then(
      () => { setCopied(true); setTimeout(() => setCopied(false), 1200); },
      () => flash("Could not copy the code", "bad"),
    );
  };

  return (
    <section className="flex flex-col gap-3 p-5">
      <div>
        <h2>Let a device in</h2>
        <p className="mt-0.5 text-text-2">When the other device asks for a code, enter this one there. It works once.</p>
      </div>
      {!connected ? (
        <p className="text-text-3">This PC's agent is not running{detail ? `: ${detail}` : ""}, so it cannot create a code.</p>
      ) : (
        <>
          <div className={`mono select-text text-[34px] leading-none tracking-[0.06em] ${expired ? "text-text-3 line-through" : "text-text"}`} aria-live="polite">
            {fresh ? group(fresh.code) : "--- ---"}
          </div>
          <p role={failed ? "alert" : undefined} className={expired || failed ? "text-bad" : "text-text-2"}>
            {failed ? `Could not create a code: ${failed}` : !fresh ? "Creating a code..." : expired ? "Expired. Make a new code." : <>Valid for <span className="mono">{clock(left)}</span></>}
          </p>
          <div className="flex gap-2">
            {failed ? (
              <button className="btn sm" onClick={request}><Icon name="refresh" />Retry</button>
            ) : (
              <>
                <button className="btn sm" aria-disabled={!fresh || expired} title={!fresh ? "Waiting for a code" : expired ? "The code expired" : undefined} onClick={copy}><Icon name={copied ? "check" : "copy"} />{copied ? "Copied" : "Copy"}</button>
                <button className="btn sm" onClick={request}><Icon name="refresh" />New code</button>
              </>
            )}
          </div>
          {fresh && !expired && (
            <div className="flex items-center gap-3">
              <Qr text={pairUri(fresh.code)} />
              <p className="max-w-[120px] text-xs text-text-2">Scan it with the phone app.</p>
            </div>
          )}
        </>
      )}
    </section>
  );
}

function Connect({ onClose }: { onClose: () => void }) {
  const [name, setName] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const devices = useStore((s) => s.devices);
  const digits = code.replace(/\D/g, "");
  const lower = name.toLowerCase();
  const taken = lower === "local" || lower === "settings" || Object.keys(devices).some((n) => n.toLowerCase() === lower);
  const valid = /^[A-Za-z0-9_-]+$/.test(name) && !taken && digits.length === 6;

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!valid || busy) return;
    setBusy(true);
    setError("");
    try {
      await pairDevice(name, digits);
      select(name);
      flash(`Paired with ${name}`);
      onClose();
    } catch (err) {
      setError(pairError(String(err).replace(/^Error:\s*/, "")) || "Pairing failed");
      setBusy(false);
    }
  };

  return (
    <form className="flex flex-col gap-3 p-5" onSubmit={submit}>
      <div>
        <h2>Connect to a device</h2>
        <p className="mt-0.5 text-text-2">When the other device shows a code, type it here.</p>
      </div>
      <label className="flex flex-col gap-1">
        <span className="text-xs text-text-2">Name for it here</span>
        <input className="field" value={name} maxLength={32} required pattern="[A-Za-z0-9_\-]+" placeholder="studio" autoComplete="off" spellCheck={false}
          onChange={(e) => setName(e.target.value.replace(/[^A-Za-z0-9_-]/g, ""))} />
        <span className={`text-xs ${taken ? "text-bad" : "text-text-2"}`}>{taken ? `"${name}" is already in use` : "Letters, numbers, - and _"}</span>
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-xs text-text-2">Code</span>
        <input className="field mono" value={group(digits)} inputMode="numeric" autoComplete="one-time-code" maxLength={7} placeholder="000 000"
          onChange={(e) => setCode(e.target.value.replace(/\D/g, "").slice(0, 6))} />
      </label>
      {error && <p role="alert" className="text-bad">{error}</p>}
      <div>
        <button className="btn primary" type="submit" aria-disabled={!valid || busy} title={valid || busy ? undefined : "Enter a free name and the 6-digit code"}>{busy ? "Pairing" : "Pair"}</button>
      </div>
    </form>
  );
}

export function PairDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Dialog open={open} onClose={onClose} label="Pair a device" className="w-[680px]">
      <div className="flex h-10 items-center border-b border-line-soft pr-2 pl-5">
        <h1 className="flex-1">Pair a device</h1>
        <button className="btn ghost sm" aria-label="Close" onClick={onClose}><Icon name="x" className="!size-3" /></button>
      </div>
      <div className="grid grid-cols-2 divide-x divide-line-soft">
        <LetIn />
        <Connect onClose={onClose} />
      </div>
    </Dialog>
  );
}
