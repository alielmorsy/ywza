import { useEffect, useRef, useState, type ReactNode } from "react";
import { send, useStore, type Config } from "../../api";
import { Icon } from "../../Icon";
import { cleanConfig } from "../localConfig";

const FPS = [15, 30, 60], KBPS = [1000, 2500, 5000, 10000];
const options = (base: number[], cur: number) => [...new Set([...base, cur])].sort((a, b) => a - b);

const Row = ({ label, children }: { label: string; children: ReactNode }) => (
  <div className="flex min-h-[34px] items-center justify-between gap-4 px-3 py-1"><span>{label}</span>{children}</div>
);

export function QualityMenu({ dev }: { dev: string }) {
  const cfg = useStore((s) => s.devices[dev]?.config);
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);

  useEffect(() => { if (open && !cfg) send(dev, { type: "get_config" }); }, [open, cfg, dev]);
  useEffect(() => {
    if (!open) return;
    const down = (e: MouseEvent) => { if (!root.current?.contains(e.target as Node)) setOpen(false); };
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") { setOpen(false); trigger.current?.focus(); } };
    document.addEventListener("mousedown", down);
    document.addEventListener("keydown", key);
    return () => { document.removeEventListener("mousedown", down); document.removeEventListener("keydown", key); };
  }, [open]);

  const save = (p: Partial<Config["stream"]>) => {
    if (!cfg) return;
    send(dev, { type: "set_config", config: cleanConfig({ ...cfg, stream: { ...cfg.stream, ...p } }) });
    send(dev, { type: "get_config" });
  };

  return (
    <div ref={root} className="relative" onBlur={(e) => { if (open && !e.currentTarget.contains(e.relatedTarget as Node | null)) setOpen(false); }}>
      <button ref={trigger} className={`btn ghost sm px-[7px] ${open ? "bg-surface-3 text-text" : ""}`} title="Stream quality" aria-label="Stream quality"
        aria-haspopup="dialog" aria-expanded={open} onClick={() => setOpen(!open)}>
        <Icon name="sliders" />
      </button>
      {open && (
        <div role="dialog" aria-label="Stream quality" tabIndex={-1}
          className="absolute top-full left-1/2 z-20 mt-2 w-[270px] -translate-x-1/2 rounded-lg border border-line bg-surface p-1 shadow-[0_8px_24px_var(--c-backdrop)] outline-none animate-[pop_.16s_var(--ease)]">
          {!cfg ? <div className="flex min-h-[34px] items-center px-3 py-1 text-text-3">Loading stream settings</div> : <>
            <Row label="Show remote cursor">
              <button className="tog" role="switch" aria-label="Show remote cursor" aria-checked={cfg.stream.cursor} onClick={() => save({ cursor: !cfg.stream.cursor })} />
            </Row>
            <Row label="Frame rate">
              <select className="field h-[26px] w-[110px]" aria-label="Frame rate" value={cfg.stream.fps} onChange={(e) => save({ fps: +e.target.value })}>
                {options(FPS, cfg.stream.fps).map((f) => <option key={f} value={f}>{f} fps</option>)}
              </select>
            </Row>
            <Row label="Bitrate">
              <select className="field h-[26px] w-[110px]" aria-label="Bitrate" value={cfg.stream.bitrate_kbps} onChange={(e) => save({ bitrate_kbps: +e.target.value })}>
                {options(KBPS, cfg.stream.bitrate_kbps).map((b) => <option key={b} value={b}>{b ? `${b / 1000} Mbps` : "Auto"}</option>)}
              </select>
            </Row>
          </>}
        </div>
      )}
    </div>
  );
}
