import { useEffect, useState } from "react";
import { mcpSet, mcpStatus, type McpClient } from "../api";
import { flash } from "../nav";
import { Row, Section } from "./Form";

export function McpClients() {
  const [clients, setClients] = useState<McpClient[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const refresh = () => mcpStatus().then(setClients).catch(() => {});
  useEffect(() => { refresh(); }, []);

  const toggle = async (c: McpClient) => {
    const install = c.state !== "installed";
    setBusy(c.id);
    try {
      await mcpSet(c.id, install);
      flash(`${c.label}: ${install ? "installed, restart it to use ywza" : "removed"}`);
    } catch (e) {
      flash(`${c.label}: ${e}`, "bad");
    }
    setBusy(null);
    refresh();
  };

  return (
    <Section h="MCP">
      {clients.map((c) => (
        <Row key={c.id} t={c.label} d={c.state === "unavailable" ? "Not found on this PC" : c.state === "installed" ? "Installed" : "Lets it see and drive this PC and paired devices"} w="w-[120px]">
          <button type="button" className={`btn sm ${c.state === "installed" ? "" : "primary"}`} disabled={busy !== null || c.state === "unavailable"} onClick={() => toggle(c)}>
            {busy === c.id ? "Working..." : c.state === "installed" ? "Remove" : "Install"}
          </button>
        </Row>
      ))}
    </Section>
  );
}
