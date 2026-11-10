import { useEffect, useRef, useState, type ReactNode } from "react";

/** Native modal dialog. Children mount only while open, so their state resets on every open. */
export function Dialog({ open, onClose, label, className = "", children }: { open: boolean; onClose: () => void; label: string; className?: string; children: ReactNode }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const d = ref.current!;
    if (open && !d.open) d.showModal();
    if (!open && d.open) d.close();
  }, [open]);
  return (
    <dialog ref={ref} aria-label={label} className={`dlg ${className}`} onClose={onClose} onMouseDown={(e) => { if (e.target === ref.current) onClose(); }}>
      {open && children}
    </dialog>
  );
}

export function Confirm({ open, title, body, action, onConfirm, onClose }: {
  open: boolean; title: string; body: string; action: string; onConfirm: () => Promise<unknown>; onClose: () => void;
}) {
  return (
    <Dialog open={open} onClose={onClose} label={title} className="w-[380px]">
      <ConfirmBody title={title} body={body} action={action} onConfirm={onConfirm} onClose={onClose} />
    </Dialog>
  );
}

function ConfirmBody({ title, body, action, onConfirm, onClose }: { title: string; body: string; action: string; onConfirm: () => Promise<unknown>; onClose: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const run = async () => {
    setBusy(true);
    setError("");
    try { await onConfirm(); onClose(); } catch (e) { setError(String(e).replace(/^Error:\s*/, "")); setBusy(false); }
  };
  return (
    <div className="flex flex-col gap-3 p-4">
      <h2 className="text-sm">{title}</h2>
      <p className="text-text-2">{body}</p>
      {error && <p role="alert" className="text-bad">{error}</p>}
      <div className="flex justify-end gap-2">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn danger" disabled={busy} onClick={run}>{busy ? "Working" : action}</button>
      </div>
    </div>
  );
}
