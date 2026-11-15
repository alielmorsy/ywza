import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Row, Tog } from "./Form";

/** Applies at once for this Windows user; it is not part of the agent's config. */
export function ExplorerMenu() {
  const [on, setOn] = useState<boolean | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => { invoke<boolean>("explorer_menu_status").then(setOn, (e) => setErr(String(e))); }, []);

  const set = (v: boolean) => {
    invoke("explorer_menu_set", { on: v }).then(() => { setOn(v); setErr(null); }, (e) => setErr(String(e)));
  };

  return (
    <Row t="Add ywza to Explorer's right-click menu" d="On Windows 11 it is under Show more options" err={err} w="w-auto">
      <Tog on={!!on} set={set} disabled={on === null} />
    </Row>
  );
}
