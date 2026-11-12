import { useEffect, useState } from "react";
import { LOCAL, removeDevice, send, useStore, type PermKey } from "../api";
import { Confirm } from "./Dialog";
import { Row, Tog } from "./Form";
import { saveConfig } from "./localConfig";

const PERMS: { k: PermKey; n: string; d?: string }[] = [
  { k: "view", n: "See the screen" },
  { k: "chat", n: "Send prompts" },
  { k: "control", n: "Use mouse and keyboard" },
  { k: "tools", n: "Run tools directly" },
  { k: "drive", n: "Drive other devices", d: "Can start chats here that use this PC's paired devices" },
  { k: "settings", n: "Change settings", d: "Can change this PC's settings, pair devices and edit access" },
];

export function Access({ dev }: { dev: string }) {
  const d = useStore((s) => s.devices[dev]);
  const local = useStore((s) => s.devices[LOCAL]);
  const config = local?.config;
  const error = useStore((s) => s.error);
  const [removing, setRemoving] = useState(false);
  const [granting, setGranting] = useState(false);
  const [pending, setPending] = useState<Partial<Record<PermKey, boolean>>>({});

  useEffect(() => setPending({}), [config, error]);

  useEffect(() => { if (!config && local?.connected) send(LOCAL, { type: "get_config" }); }, [config, local?.connected]);

  const perms = d?.permissions;
  const peer = config?.peers.find((p) => (d?.nodeId && p.id === d.nodeId) || p.name === dev);
  const toggle = (k: PermKey, v: boolean) => {
    if (!config || !peer) return;
    if (k === "settings" && v && !granting) { setGranting(true); return; }
    setPending((p) => ({ ...p, [k]: v }));
    saveConfig({ ...config, llm: { ...config.llm, api_key: "" }, peers: config.peers.map((p) => (p === peer ? { ...p, ...pending, [k]: v } : p)) }, `Updated access for ${dev}`);
  };

  return (
    <div className="h-full overflow-auto px-6 py-5">
      <div className="mx-auto grid max-w-[820px] grid-cols-1 items-start gap-8">
        <section className="order-2">
          <h2>What this PC may do on {dev}</h2>
          <p className="mb-2 text-xs text-text-2">Set on {dev}. Change it there.</p>
          {perms === undefined ? (
            <p className="text-text-3">Shown once {dev} is online.</p>
          ) : (
            <div className="flex flex-wrap gap-1">
              {PERMS.map((p) => {
                const on = !perms || perms[p.k];
                return <span key={p.k} className={`tag ${on ? "on" : "off"}`}>{p.n}<span className="sr-only">{on ? " allowed" : " not allowed"}</span></span>;
              })}
            </div>
          )}
        </section>

        <section>
          <h2>What {dev} may do on this PC</h2>
          {peer && <p className="text-xs text-text-2">You decide. Changes save right away.</p>}
          {!config ? (
            <p className="text-text-3">{local?.connected ? "Loading access..." : "This PC's agent is not running, so access cannot be edited."}</p>
          ) : !peer ? (
            <p className="text-text-3">{dev} is not in this PC's peer list, so it has no access here and there is nothing to edit.</p>
          ) : (
            <div className="mt-2 rounded-md border border-line bg-surface">
              {PERMS.map((p) => (
                <Row key={p.k} t={p.n} d={p.d} w="w-auto">
                  <Tog on={pending[p.k] ?? !!peer[p.k]} set={(v) => toggle(p.k, v)} />
                </Row>
              ))}
            </div>
          )}
        </section>
      </div>

      <div className="mx-auto mt-8 flex max-w-[820px] items-center gap-4 border-t border-line-soft pt-4">
        <p className="flex-1 text-text-3">Removing unpairs {dev}. It needs a new code to connect again.</p>
        <button className="btn danger" onClick={() => setRemoving(true)}>Remove device</button>
      </div>
      <Confirm open={granting} title={`Let ${dev} change settings?`} action="Allow"
        body={`${dev} could change this PC's AI and approval rules, pair new devices and edit everyone's access, its own included.`}
        onConfirm={async () => toggle("settings", true)} onClose={() => setGranting(false)} />
      <Confirm open={removing} title={`Remove ${dev}?`} action="Remove"
        body="It is unpaired and loses all access to this PC. To use it again you pair it with a new code."
        onConfirm={() => removeDevice(dev)} onClose={() => setRemoving(false)} />
    </div>
  );
}
