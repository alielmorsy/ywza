import { useEffect, useLayoutEffect, useRef, type KeyboardEvent } from "react";

const ENABLED = 'button:not([aria-disabled="true"])';

export interface MenuItem { label: string; run: () => void; disabled?: string | boolean; danger?: boolean }

export function ContextMenu({ x, y, items, onClose }: { x: number; y: number; items: MenuItem[]; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const prev = useRef(document.activeElement as HTMLElement | null);

  useLayoutEffect(() => {
    const el = ref.current!;
    el.style.left = Math.max(4, Math.min(x, innerWidth - el.offsetWidth - 6)) + "px";
    el.style.top = Math.max(4, Math.min(y, innerHeight - el.offsetHeight - 6)) + "px";
    el.querySelector<HTMLElement>(ENABLED)?.focus();
  }, [x, y]);

  useEffect(() => {
    const down = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) onClose(); };
    addEventListener("mousedown", down);
    addEventListener("blur", onClose);
    return () => { removeEventListener("mousedown", down); removeEventListener("blur", onClose); prev.current?.focus?.(); };
  }, []);

  const keys = (e: KeyboardEvent) => {
    if (e.key === "Escape" || e.key === "Tab") { e.preventDefault(); return onClose(); }
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) return;
    e.preventDefault();
    const b = [...ref.current!.querySelectorAll<HTMLElement>(ENABLED)];
    const i = b.indexOf(document.activeElement as HTMLElement);
    const next = e.key === "Home" ? 0 : e.key === "End" ? b.length - 1 : (i + (e.key === "ArrowDown" ? 1 : -1) + b.length) % b.length;
    b[next]?.focus();
  };

  return (
    <div ref={ref} role="menu" className="menu" onKeyDown={keys}>
      {items.map((it) => (
        <button key={it.label} role="menuitem" className={it.danger ? "danger" : ""} aria-disabled={!!it.disabled}
          title={typeof it.disabled === "string" ? it.disabled : undefined}
          onClick={() => { if (it.disabled) return; onClose(); it.run(); }}>
          {it.label}
        </button>
      ))}
    </div>
  );
}
