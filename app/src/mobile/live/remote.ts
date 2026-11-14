// Input sent to a device's live view from the phone.
import { send, type InputEvent } from "../../api";

export const emit = (dev: string, event: InputEvent) => send(dev, { type: "input", event });

/** Holds `keys` down in order and lets go in reverse: ["ctrl", "shift", "p"]. */
export function press(dev: string, keys: string[]) {
  keys.forEach((key) => emit(dev, { type: "key", key, down: true }));
  [...keys].reverse().forEach((key) => emit(dev, { type: "key", key, down: false }));
}

export function click(dev: string, at: { x: number; y: number }, button: "left" | "right" = "left") {
  emit(dev, { type: "mouse_move", x: at.x, y: at.y });
  emit(dev, { type: "mouse_button", button, down: true });
  emit(dev, { type: "mouse_button", button, down: false });
}
