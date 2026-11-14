import { chipName, type LensCtl } from "../../views/live/lens";
import { LensEditor } from "../../views/live/LensChips";

const LIMIT = 30;

/** Below the screen: what the lens follows, a search over its elements, and the editor for the chosen text field. */
export function LensPanel({ lens, streaming }: { lens: LensCtl; streaming: boolean }) {
  const rows = lens.filtering ? lens.shown.slice(0, LIMIT) : [];
  const status = !streaming ? "Waiting for the screen" : !lens.lens ? lens.quiet ? "No answer. This device's ywza agent may be too old for the lens." : "Looking for the focused window" : `${lens.lens.title || "Window"} · ${lens.all.length} elements`;
  return (
    <section className="flex flex-col gap-2 rounded-xl border border-line bg-surface px-3 py-2.5" aria-label="Smart lens">
      <div className="flex items-baseline justify-between gap-2">
        <small className="text-[10.5px] tracking-[.05em] text-text-2 uppercase">Lens</small>
        <span className="min-w-0 truncate text-[11px] text-text-2" role="status">{status}</span>
      </div>
      {!lens.canAct && <p className="text-xs text-warn">View only: this device does not allow control, so elements cannot be tapped.</p>}
      {lens.editEl && <LensEditor mobile el={lens.editEl} onType={lens.type} onClose={() => lens.setEditing(null)} />}
      <input type="search" value={lens.query} onChange={(e) => lens.setQuery(e.target.value)} placeholder="Find a button or field" aria-label="Find an element by name or role"
        autoComplete="off" className="h-10 rounded-[10px] border border-line bg-chrome px-3 outline-none focus:border-accent focus:shadow-[0_0_0_3px_var(--c-accent-soft)]" />
      {lens.filtering && (
        <ul className="flex flex-col" aria-label="Matches">
          {rows.map((el) => (
            <li key={el.id}>
              <button type="button" aria-disabled={!lens.canAct} className="flex min-h-11 w-full items-center gap-2 text-left text-[13px] aria-disabled:opacity-50" onClick={() => lens.tap(el)}>
                <span className="mono flex-none rounded-[3px] bg-surface-3 px-1 text-[10px] text-text-2">{el.editable ? "field" : el.role}</span>
                <span className="min-w-0 flex-1 truncate">{chipName(el)}</span>
              </button>
            </li>
          ))}
          {!rows.length && <li className="py-1 text-xs text-text-2">Nothing matches.</li>}
          {lens.shown.length > LIMIT && <li className="py-1 text-xs text-text-2">{lens.shown.length - LIMIT} more. Type more to narrow it.</li>}
        </ul>
      )}
      {!lens.filtering && <p className="text-xs text-text-2">Tap a box on the screen, or search by name.</p>}
    </section>
  );
}
