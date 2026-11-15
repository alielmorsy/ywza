import { useState } from "react";
import { dockShortcut, type DockKey } from "../api";
import { SHORTCUTS, saveShortcut, savedShortcut } from "../dock/shortcut";
import { Row } from "./Form";

/** Applies at once and is kept on this PC; it is not part of the agent's config. */
export function DockShortcut() {
  const [key, setKey] = useState(savedShortcut);
  const [err, setErr] = useState<string | null>(null);

  const pick = (k: DockKey) => {
    dockShortcut(k).then(
      () => { saveShortcut(k); setKey(k); setErr(null); },
      (e) => setErr(`${SHORTCUTS.find(([x]) => x === k)?.[1]} is taken: ${e}`),
    );
  };

  return (
    <Row t="Command box shortcut" d="Opens the command box from anywhere" err={err} w="w-[240px]">
      <select className="field" value={key} onChange={(e) => pick(e.target.value as DockKey)}>
        {SHORTCUTS.map(([k, name]) => <option key={k} value={k}>{name}</option>)}
      </select>
    </Row>
  );
}
