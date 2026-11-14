import { Icon } from "../../Icon";

/** What the agent saw change after the human took over, collapsed to one line. */
export function ResumedRow({ digest }: { digest: string }) {
  return (
    <details className="group text-xs text-text-2">
      <summary className="flex w-fit cursor-pointer list-none items-center gap-2 hover:text-text [&::-webkit-details-marker]:hidden">
        <Icon name="check" className="size-3.5" />Resumed
        <Icon name="chev" className="transition-transform duration-200 ease-out-expo group-open:rotate-180" />
      </summary>
      <div className="mono mt-1.5 max-h-40 overflow-auto rounded-md bg-chrome px-2 py-1.5 leading-[1.6] whitespace-pre-wrap break-words select-text">{digest}</div>
    </details>
  );
}
