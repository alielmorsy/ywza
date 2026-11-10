import { useState } from "react";
import { Icon } from "../Icon";

const CMD = "ywza-agent";

/** Whole-window card while this PC's agent is unreachable. The app keeps retrying in the background. */
export function ServiceDown({ detail }: { detail?: string }) {
  const [copied, setCopied] = useState(false);
  const copy = () => navigator.clipboard.writeText(CMD).then(() => { setCopied(true); setTimeout(() => setCopied(false), 1200); }).catch(() => {});
  return (
    <div className="grid h-full place-items-center bg-ground p-6">
      <section role="alert" aria-live="assertive"
        className="down-card flex w-[min(400px,100%)] flex-col items-center gap-5 rounded-lg border border-line bg-surface px-8 pt-9 pb-7 text-center shadow-[0_24px_60px_var(--c-backdrop)]">
        <div className="down-sonar" aria-hidden>
          <span /><span /><span />
          <div className="down-core"><Icon name="server" /></div>
        </div>
        <div className="flex flex-col gap-1.5">
          <h1 className="text-[17px] font-semibold">Service is down</h1>
          <p className="text-text-2">ywza can't reach the agent on this PC. Start it and this window comes back on its own.</p>
        </div>
        <button type="button" onClick={copy} title="Copy command"
          className="flex h-9 w-full items-center gap-2 rounded-md border border-line bg-chrome px-3 text-left font-mono transition-colors hover:border-line-hover">
          <span className="text-text-3">&gt;</span>
          <span className="flex-1 select-text">{CMD}</span>
          <span className="text-text-2"><Icon name={copied ? "check" : "copy"} /></span>
        </button>
        <p className="flex items-center gap-2 text-xs text-text-2">
          <span className="dot off pulse" />Looking for the service
        </p>
        {detail && <p className="max-w-full truncate text-xs text-text-3" title={detail}>{detail}</p>}
      </section>
    </div>
  );
}
