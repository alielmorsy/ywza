import { useEffect, useState } from "react";
import { dockShortcut, type DockKey } from "../api";
import { SHORTCUTS, saveShortcut, savedShortcut } from "../dock/shortcut";
import { Row } from "./Form";

/** Applies at once and is kept on this PC; it is not part of the agent's config. */
export function DockShortcut() {
  const [key, setKey] = useState(savedShortcut);
  const [err, setErr] = useState<string | null>(null);

  // Surfaces a shortcut that failed to register at startup.
  useEffect(() => { dockShortcut(savedShortcut()).then(() => setErr(null), (e) => setErr(String(e))); }, []);

  const pick = (k: DockKey) => {
    dockShortcut(k).then(
      () => { saveShortcut(k); setKey(k); setErr(null); },
      (e) => setErr(String(e)),
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
