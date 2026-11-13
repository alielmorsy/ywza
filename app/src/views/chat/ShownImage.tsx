import { useEffect, useState } from "react";
import { Icon } from "../../Icon";
import { createPortal } from "react-dom";

/** A screenshot the assistant showed the user (`show_screenshot`). Tap to see it full size, tap again to close. */
export function ShownImage({ png, className = "" }: { png: string; className?: string }) {
  const [open, setOpen] = useState(false);
  const src = `data:image/png;base64,${png}`;
  useEffect(() => {
    if (!open) return;
    // A history entry lets the phone's back gesture (the WebView goes back) close it.
    history.pushState({ shot: true }, "");
    const pop = () => setOpen(false);
    const k = (e: KeyboardEvent) => { if (e.key === "Escape") history.back(); };
    addEventListener("popstate", pop);
    addEventListener("keydown", k);
    return () => { removeEventListener("popstate", pop); removeEventListener("keydown", k); };
  }, [open]);
  const close = () => history.back(); // pops our entry, and popstate closes
  return (
    <>
      <button type="button" className={`block w-fit max-w-full overflow-hidden rounded-md border border-line-soft bg-backdrop ${className}`}
        onClick={() => setOpen(true)} aria-label="Screenshot, open full size">
        <img src={src} alt="Screenshot shown by the assistant" className="block max-h-[50vh] w-auto max-w-full object-contain" />
      </button>
      {/* On body: inside the phone's chat sheet a fixed layer stays under the tab bar. */}
      {open && createPortal(
        <div role="dialog" aria-modal="true" aria-label="Screenshot" className="fixed inset-0 z-[70] grid place-items-center bg-black/90 p-2" onClick={close}>
          <img src={src} alt="Screenshot shown by the assistant" className="max-h-full max-w-full object-contain" />
          <button type="button" autoFocus onClick={(e) => { e.stopPropagation(); close(); }}
            className="absolute top-[max(12px,env(safe-area-inset-top))] left-3 flex h-11 items-center gap-1.5 rounded-full border border-white/25 bg-black/70 px-4 text-sm text-white">
            <Icon name="back" />Back
          </button>
        </div>,
        document.body,
      )}
    </>
  );
}
