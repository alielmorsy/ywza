import { useEffect, useRef, type HTMLAttributes, type KeyboardEvent, type PointerEvent, type RefObject } from "react";
import { send, type InputEvent } from "../../api";
import { BUTTONS, keyName, pointOn, wheelNotches, type Region } from "./input";

/** Canvas props that forward pointer, wheel and key input to the device. Empty while not `enabled`. */
export function useRemoteInput(
  dev: string, canvas: RefObject<HTMLCanvasElement | null>, region: () => Region, enabled: boolean,
): HTMLAttributes<HTMLCanvasElement> {
  const held = useRef(new Set<string>()); // keys down, released on blur
  const move = useRef<InputEvent | null>(null);
  const emit = (event: InputEvent) => send(dev, { type: "input", event });
  const release = () => { held.current.forEach((key) => emit({ type: "key", key, down: false })); held.current.clear(); };
  const at = (e: { clientX: number; clientY: number }) => { const c = canvas.current; return c ? pointOn(c, e, region()) : null; };
  useEffect(() => (enabled ? release : undefined), [enabled, dev]);
  if (!enabled) return {};

  const button = (e: PointerEvent<HTMLCanvasElement>, down: boolean) => {
    const b = BUTTONS[e.button], p = at(e), t = e.currentTarget;
    if (!b || !p || (down ? !p.inside : !t.hasPointerCapture(e.pointerId))) return;
    if (down) { t.setPointerCapture(e.pointerId); t.focus(); }
    move.current = null;
    emit({ type: "mouse_move", x: p.x, y: p.y });
    emit({ type: "mouse_button", button: b, down });
  };
  const key = (e: KeyboardEvent<HTMLCanvasElement>, down: boolean) => {
    e.preventDefault(); e.stopPropagation(); // app shortcuts (Ctrl+W, Ctrl+Tab) go to the device
    if (down && e.ctrlKey && e.altKey && e.key === "Home") { e.currentTarget.blur(); return; } // keyboard way out
    const k = keyName(e);
    if (!k) return;
    if (down) held.current.add(k); else held.current.delete(k);
    emit({ type: "key", key: k, down });
  };

  return {
    tabIndex: 0,
    "aria-label": "Remote screen. Ctrl+Alt+Home gives the keyboard back to this app.",
    title: "Ctrl+Alt+Home gives the keyboard back to this app",
    onContextMenu: (e) => e.preventDefault(),
    onPointerMove(e) { // at most one move per animation frame
      const p = at(e);
      if (!p) return;
      if (!move.current) requestAnimationFrame(() => { if (move.current) emit(move.current); move.current = null; });
      move.current = { type: "mouse_move", x: p.x, y: p.y };
    },
    onPointerDown: (e) => button(e, true),
    onPointerUp: (e) => button(e, false),
    onWheel: (e) => emit({ type: "wheel", ...wheelNotches(e) }),
    onKeyDown: (e) => key(e, true),
    onKeyUp: (e) => key(e, false),
    onBlur: release,
  };
}
