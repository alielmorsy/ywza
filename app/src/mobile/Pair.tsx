import { useState, type FormEvent } from "react";
import { LOCAL, pairDevice, useStore } from "../api";
import { Icon } from "../Icon";
import { setPairing, setTab, toast } from "./store";
import { pairCode } from "../views/pairCode";
import { Back } from "./ui";

const group = (c: string) => c.replace(/(\d{3})(?=\d)/, "$1 ");

/** Backend pairing errors speak to CLI users; say what to do here instead. */
const pairError = (e: string) =>
  /no agent answered|could not reach/i.test(e) ? "Could not reach the other device. Check the code, or make a new one there and try again."
  : /wrong or expired code|code expired/i.test(e) ? "Wrong or expired code. Make a new one on the other device and try again."
  : e;

const KEYS = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "scan", "0", "del"] as const;

/** The QR on the other device holds ywza://pair/<code>. The plugin loads on demand so the desktop preview still starts. */
async function scanCode() {
  const { scan, Format, requestPermissions } = await import("@tauri-apps/plugin-barcode-scanner");
  if ((await requestPermissions()) !== "granted") throw new Error("Camera access is off. Allow it in Settings, or type the code.");
  const digits = pairCode((await scan({ formats: [Format.QRCode] })).content);
  if (!digits) throw new Error("That QR code is not a pairing code. Open Let a device in on the other device.");
  return digits;
}

export function Pair() {
  const devices = useStore((s) => s.devices);
  const [name, setName] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const lower = name.toLowerCase();
  const taken = lower === LOCAL || lower === "settings" || Object.keys(devices).some((n) => n.toLowerCase() === lower);
  const valid = /^[A-Za-z0-9_-]+$/.test(name) && !taken && code.length === 6;

  const press = (k: (typeof KEYS)[number]) => {
    setError("");
    if (k === "del") setCode(code.slice(0, -1));
    else if (k !== "scan") setCode((code + k).slice(0, 6));
    else {
      setBusy(true);
      scanCode().then(setCode, (e) => setError(String(e?.message ?? e).replace(/^Error:\s*/, ""))).finally(() => setBusy(false));
    }
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!valid || busy) return;
    setBusy(true);
    setError("");
    try {
      await pairDevice(name, code);
      toast(`Paired with ${name}`);
      setPairing(false);
      setTab("wall");
    } catch (err) {
      setError(pairError(String(err).replace(/^Error:\s*/, "")) || "Pairing failed");
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit} className="absolute inset-0 z-50 flex flex-col bg-ground pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)]" style={{ animation: "m-rise .25s var(--ease)" }} aria-label="Pair a device">
      <header className="flex flex-none items-center gap-1 px-2 py-1">
        <Back onClick={() => setPairing(false)} label="Close" />
        <h1 className="text-[17px] font-semibold">Pair a device</h1>
      </header>
      <div className="m-scroll flex flex-1 flex-col gap-4 px-5 pt-2 pb-3">
        <p className="text-text-2">On the other device open Pair a device, then Let a device in. Type the 6-digit code it shows, or scan its QR.</p>
        <label className="flex flex-col gap-1.5">
          <span className="text-xs text-text-2">Name for it here</span>
          <input value={name} maxLength={32} placeholder="studio" autoComplete="off" autoCapitalize="none" spellCheck={false} aria-invalid={taken}
            onChange={(e) => setName(e.target.value.replace(/[^A-Za-z0-9_-]/g, ""))}
            className="h-12 rounded-[10px] border border-line bg-chrome px-3 outline-none placeholder:text-text-3 focus:border-accent focus:shadow-[0_0_0_3px_var(--c-accent-soft)] aria-[invalid=true]:border-bad" />
          <span className={`text-xs ${taken ? "text-bad" : "text-text-2"}`}>{taken ? `"${name}" is already in use` : "Letters, numbers, - and _"}</span>
        </label>
        <div className="flex flex-col items-center gap-1 py-1" aria-live="polite">
          <span className="text-xs text-text-2">Code</span>
          <div className="mono text-[40px] leading-none tracking-[.06em]" aria-label={code ? `Code ${code.split("").join(" ")}` : "No code yet"}>
            {group(code.padEnd(6, "-"))}
          </div>
        </div>
        {error && <p role="alert" className="text-center text-bad">{error}</p>}
        <div className="mx-auto grid w-full max-w-[320px] grid-cols-3 gap-2.5">
          {KEYS.map((k) => (
            <button key={k} type="button" disabled={busy && k === "scan"} onClick={() => press(k)}
              aria-label={k === "del" ? "Delete" : k === "scan" ? "Scan QR code" : k}
              className="grid h-14 place-items-center rounded-xl border border-line-soft bg-surface text-[22px] transition-colors active:bg-surface-3">
              {k === "del" ? <Icon name="backspace" className="!size-6" /> : k === "scan" ? <Icon name="qr" className="!size-6" /> : k}
            </button>
          ))}
        </div>
      </div>
      <div className="flex-none px-5 pt-1 pb-4">
        <button type="submit" className="m-btn primary w-full" aria-disabled={!valid || busy}>{busy ? "Working" : "Pair"}</button>
      </div>
    </form>
  );
}
