import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Icon } from "../../Icon";
import { keyName } from "../../views/live/input";
import { emit, press } from "./remote";

type Mod = "ctrl" | "alt" | "shift" | "win";
const MODS: Mod[] = ["ctrl", "alt", "shift", "win"];
/** 0 off, 1 for the next key, 2 locked. */
type Mods = Record<Mod, 0 | 1 | 2>;
const OFF: Mods = { ctrl: 0, alt: 0, shift: 0, win: 0 };

const KEYS: [string, string][] = [
  ["esc", "Esc"], ["tab", "Tab"], ["left", "←"], ["up", "↑"], ["down", "↓"], ["right", "→"],
  ["home", "Home"], ["end", "End"], ["pageup", "PgUp"], ["pagedown", "PgDn"], ["backspace", "⌫"], ["delete", "Del"], ["enter", "⏎"],
  ...Array.from({ length: 12 }, (_, i): [string, string] => [`f${i + 1}`, `F${i + 1}`]),
];
const SYMBOLS = [..."{}()[]<>=;:'\"`~|\\/_-+*&!?$#%^@"];
/** Coding shortcuts: [label, keys, what it is]. */
const SHORTCUTS: [string, string[], string][] = [
  ["^C", ["ctrl", "c"], "Copy, or stop the running command"],
  ["^V", ["ctrl", "v"], "Paste"],
  ["^Z", ["ctrl", "z"], "Undo"],
  ["^Y", ["ctrl", "y"], "Redo"],
  ["^S", ["ctrl", "s"], "Save"],
  ["^F", ["ctrl", "f"], "Find"],
  ["^A", ["ctrl", "a"], "Select all"],
  ["^/", ["ctrl", "/"], "Toggle comment"],
  ["^P", ["ctrl", "p"], "Go to file"],
  ["⇧^P", ["ctrl", "shift", "p"], "Command palette"],
  ["^`", ["ctrl", "`"], "Toggle terminal"],
  ["^D", ["ctrl", "d"], "End of input, or next match"],
  ["^L", ["ctrl", "l"], "Clear the terminal"],
  ["^R", ["ctrl", "r"], "Search shell history"],
  ["⇧⏎", ["shift", "enter"], "New line in a chat box"],
  ["^⏎", ["ctrl", "enter"], "Run or submit"],
  ["^⌫", ["ctrl", "backspace"], "Delete word"],
  ["^Tab", ["ctrl", "tab"], "Next tab"],
  ["Alt⇥", ["alt", "tab"], "Switch window"],
  ["F5", ["f5"], "Run or refresh"],
  ["F12", ["f12"], "Go to definition"],
];
/** Zero-width spaces the live input keeps, so a soft keyboard's backspace has something to delete and fires an event. */
const PAD = "​".repeat(8);
const drafts = new Map<string, string>();

/** A pad key. Touching it must not take focus from the typing field, or the soft keyboard closes. */
function Key({ on, locked, label, title, wide, onDown, onTap, after }: { on?: boolean; locked?: boolean; label: ReactNode; title?: string; wide?: boolean; onDown: () => void; onTap: () => void; after: () => void }) {
  return (
    <button type="button" tabIndex={-1} title={title} aria-label={title} aria-pressed={on}
      className={`mono grid h-9 flex-none place-items-center rounded-lg border px-2.5 text-[13px] ${wide ? "min-w-12" : "min-w-9"} ${on ? "border-primary bg-primary text-ink" : "border-line bg-surface text-text active:bg-surface-3"} ${locked ? "underline decoration-2 underline-offset-4" : ""}`}
      onPointerDown={onDown} onMouseDown={(e) => e.preventDefault()} onClick={() => { onTap(); after(); }}>
      {label}
    </button>
  );
}

/** Leading common length of two strings. */
const common = (a: string, b: string) => { let i = 0; while (i < a.length && i < b.length && a[i] === b[i]) i++; return i; };

/**
 * The phone keyboard for a remote screen. "Live" sends every keystroke as it is typed (soft
 * keyboards report text, not keys, so the input is diffed); "Draft" sends a block at once,
 * pasted on the device so editors keep its indentation. Modifiers stick for the next key,
 * or stay on after a second tap.
 */
export function KeyPad({ dev, onClose, autoFocus }: { dev: string; onClose: () => void; autoFocus: boolean }) {
  const [mods, setMods] = useState<Mods>(OFF);
  const [row, setRow] = useState<"sym" | "cut">("cut");
  const [draft, setDraft] = useState(false);
  const [text, setText] = useState(drafts.get(dev) ?? "");
  const [echo, setEcho] = useState("");
  const live = useRef<HTMLInputElement>(null);
  const area = useRef<HTMLTextAreaElement>(null);
  const last = useRef(PAD);
  const composing = useRef(false);
  const kept = useRef(false); // the soft keyboard was up when a button was touched

  useEffect(() => { if (autoFocus) live.current?.focus({ preventScroll: true }); }, [autoFocus]);
  useEffect(() => { drafts.set(dev, text); }, [dev, text]);

  const held = (MODS.filter((m) => mods[m]));
  const used = () => setMods((m) => ({ ctrl: m.ctrl === 2 ? 2 : 0, alt: m.alt === 2 ? 2 : 0, shift: m.shift === 2 ? 2 : 0, win: m.win === 2 ? 2 : 0 }));
  const shown = (s: string) => setEcho((e) => (e + s).slice(-40));
  const key = (k: string, label = k) => { press(dev, [...held, k]); used(); shown(held.length ? ` ${[...held, label].join("+")} ` : label); };
  /** A typed character: text, or a key when modifiers are on (Ctrl+C). */
  const char = (c: string) => {
    if (held.length) return key(c.toLowerCase(), c);
    emit(dev, { type: "text", text: c });
    shown(c);
  };
  const typed = (s: string) => {
    if (!s) return;
    if (held.length && [...s].length === 1) return char(s);
    emit(dev, { type: "text", text: s });
    shown(s);
  };

  const sync = (v: string) => {
    const old = last.current, i = common(old, v);
    for (let n = old.length - i; n > 0; n--) key("backspace", "⌫");
    typed(v.slice(i).replaceAll("​", ""));
    last.current = v;
    if (!composing.current && v !== PAD) { // start over, so the field never fills up
      last.current = PAD;
      if (live.current) live.current.value = PAD;
    }
  };
  const liveKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing || e.key === "Unidentified" || e.key === "Process") return;
    const name = keyName(e);
    const chord = e.ctrlKey || e.altKey || e.metaKey;
    // Hardware keys and Enter go as keys; printable characters arrive through `input`.
    if (!name || (!chord && e.key.length === 1) || ["ctrl", "shift", "alt", "win"].includes(name)) return;
    e.preventDefault();
    const extra = [e.ctrlKey && "ctrl", e.altKey && "alt", e.shiftKey && name.length > 1 && "shift", e.metaKey && "win"].filter((m): m is string => !!m && !held.includes(m as Mod));
    press(dev, [...held, ...extra, name]);
    used();
    shown(name === "enter" ? "⏎" : name === "backspace" ? "⌫" : ` ${name} `);
  };

  const sendDraft = (enter: boolean) => {
    if (text) emit(dev, { type: "text", text });
    if (enter) press(dev, ["enter"]);
    setText("");
    area.current?.focus();
  };

  // Pad keys give focus back to whichever field had it, so the soft keyboard stays up.
  const onDown = () => { const a = document.activeElement; kept.current = a === live.current || a === area.current; };
  const after = () => { if (kept.current) (draft ? area : live).current?.focus({ preventScroll: true }); };

  return (
    <div className="flex flex-none flex-col gap-1.5 border-t border-line bg-chrome px-2 pt-1.5 pb-[max(6px,env(safe-area-inset-bottom))]" role="group" aria-label="Keyboard for the remote screen">
      <div className="m-scroll-x flex gap-1">
        {MODS.map((m) => (
          <Key onDown={onDown} after={after} key={m} on={mods[m] > 0} locked={mods[m] === 2} wide title={`${m} (tap twice to lock)`} label={{ ctrl: "Ctrl", alt: "Alt", shift: "Shift", win: "Win" }[m]}
            onTap={() => setMods((x) => ({ ...x, [m]: ((x[m] + 1) % 3) as 0 | 1 | 2 }))} />
        ))}
        <span className="w-1 flex-none" />
        {KEYS.map(([k, label]) => <Key onDown={onDown} after={after} key={k} label={label} title={k} onTap={() => key(k, label)} />)}
      </div>
      <div className="m-scroll-x flex gap-1">
        <Key onDown={onDown} after={after} on label={row === "cut" ? "^" : "{ }"} title={row === "cut" ? "Showing shortcuts, tap for symbols" : "Showing symbols, tap for shortcuts"} onTap={() => setRow(row === "cut" ? "sym" : "cut")} />
        {row === "sym"
          ? SYMBOLS.map((c) => <Key onDown={onDown} after={after} key={c} label={c} title={c} onTap={() => char(c)} />)
          : SHORTCUTS.map(([label, keys, what]) => <Key onDown={onDown} after={after} key={label} wide label={label} title={what} onTap={() => { press(dev, [...new Set([...held, ...keys])]); used(); shown(` ${label} `); }} />)}
      </div>
      <div className="flex items-end gap-1.5">
        <div className="flex flex-none flex-col overflow-hidden rounded-lg border border-line text-[11px]" role="radiogroup" aria-label="Typing mode">
          {(["Live", "Draft"] as const).map((m) => (
            <button key={m} type="button" role="radio" aria-checked={draft === (m === "Draft")} onClick={() => setDraft(m === "Draft")}
              className={`h-[22px] px-2 ${draft === (m === "Draft") ? "bg-surface-3 text-text" : "text-text-2"}`}>{m}</button>
          ))}
        </div>
        {draft ? (
          <>
            <textarea ref={area} value={text} onChange={(e) => setText(e.target.value)} rows={3} placeholder="Write code or a prompt, then send it in one go"
              autoCapitalize="off" autoCorrect="off" spellCheck={false} aria-label="Draft"
              className="mono min-h-11 min-w-0 flex-1 resize-none rounded-lg border border-line bg-surface px-2 py-1.5 text-[13px] outline-none focus:border-primary" />
            <div className="flex flex-none flex-col gap-1">
              <Key onDown={onDown} after={after} label="Send" title="Send the draft" onTap={() => sendDraft(false)} wide />
              <Key onDown={onDown} after={after} label="Send ⏎" title="Send the draft, then Enter" onTap={() => sendDraft(true)} wide />
            </div>
          </>
        ) : (
          <label className="relative min-w-0 flex-1">
            <input ref={live} defaultValue={PAD} aria-label="Type on the remote screen"
              autoCapitalize="off" autoCorrect="off" autoComplete="off" spellCheck={false} enterKeyHint="enter"
              onInput={(e) => sync(e.currentTarget.value)}
              onCompositionStart={() => { composing.current = true; }}
              onCompositionEnd={(e) => { composing.current = false; sync(e.currentTarget.value); }}
              onKeyDown={liveKey}
              onFocus={(e) => { const n = e.currentTarget.value.length; e.currentTarget.setSelectionRange(n, n); }}
              className="mono h-11 w-full rounded-lg border border-line bg-surface px-2 text-transparent caret-transparent outline-none focus:border-primary" />
            <span className="mono pointer-events-none absolute inset-y-0 right-2 left-2 flex items-center justify-end overflow-hidden text-[13px] whitespace-pre text-text-2" aria-hidden>
              {echo || "Tap here, type: keys go straight to the screen"}
            </span>
          </label>
        )}
        <button type="button" className="m-btn sm flex-none !px-2.5" aria-label="Close keyboard" onClick={onClose}><Icon name="x" /></button>
      </div>
    </div>
  );
}
