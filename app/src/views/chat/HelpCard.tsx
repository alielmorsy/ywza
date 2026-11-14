import { useState } from "react";
import { Icon } from "../../Icon";
import { LOCAL, resume, type Help, type HelpKind } from "../../api";
import { openDevice, setUi } from "../../nav";
import { setMonitor } from "../live/useLiveStream";

export const HELP_TITLE: Record<HelpKind, string> = {
  otp: "verification code", uac: "permission prompt", credentials: "credentials", captcha: "captcha", login: "login", stuck: "agent is stuck",
};

/** The agent paused on a blocker. `open`: the latest unanswered one, the only one with buttons. `dev`: the device it works on. */
export function HelpCard({ h, open, dev, session }: { h: Help; open: boolean; dev: string; session: string }) {
  const [note, setNote] = useState("");
  const [sent, setSent] = useState(false); // until the agent answers with `resumed`

  if (!open) {
    return (
      <div className="flex min-w-0 items-center gap-2 text-xs text-text-2">
        <Icon name="check" className="size-3.5" />
        <span className="min-w-0 truncate">Needed you: {HELP_TITLE[h.kind]}</span>
      </div>
    );
  }

  const takeOver = () => {
    if (h.monitor !== undefined) setMonitor(dev, h.monitor);
    openDevice(dev, "live");
    setUi(dev, { dock: true });
  };
  const go = () => { setSent(true); setTimeout(() => setSent(false), 8000); resume(LOCAL, session, note); };

  return (
    <div role="group" aria-label={`Needs you: ${HELP_TITLE[h.kind]}`} className="rounded-md border border-warn/45 bg-warn-soft px-3 py-2.5">
      <div className="flex items-center gap-2 font-semibold text-warn"><Icon name="shield" /><span>Needs you: {HELP_TITLE[h.kind]}</span></div>
      <div className="mt-1.5 break-words text-text select-text">{h.reason}</div>
      {h.title && <div className="mt-1 truncate text-xs text-text-2">Window: {h.title}</div>}
      <div className="mt-2.5 flex flex-wrap items-center gap-1.5">
        <button className="btn sm" title="Open the live view to fix it yourself" onClick={takeOver}>Take over</button>
        <input className="field h-[26px] min-w-0 flex-1 text-xs" aria-label="Note for the agent" placeholder="Optional note" value={note} onChange={(e) => setNote(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter" && !sent) go(); }} />
        <button className="btn sm primary" disabled={sent} title="Tell the agent to continue" onClick={go}>Resume</button>
      </div>
    </div>
  );
}
