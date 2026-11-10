import { Icon, type IconName } from "../../Icon";

export function Notice({ title, hint, icon }: { title: string; hint?: string; icon?: IconName }) {
  return (
    <div className="grid h-full place-items-center p-6 text-center" role="status">
      <div className="flex flex-col items-center gap-1.5">
        {icon && <Icon name={icon} className="size-5 text-faint" />}
        <p className="text-text-2">{title}</p>
        {hint && <p className="max-w-[360px] text-xs text-text-3">{hint}</p>}
      </div>
    </div>
  );
}

/** Why the live view of a device is not available, or null when it is. */
export function unavailable(name: string, connected: boolean, canView: boolean, detail?: string) {
  if (!connected) return <Notice icon="unlink" title={`${name} is offline`} hint={detail || "The live view resumes when it reconnects."} />;
  if (!canView) return <Notice icon="eye" title={`Viewing the screen of ${name} is not allowed`} hint="Ask its owner to allow viewing for this device." />;
  return null;
}
