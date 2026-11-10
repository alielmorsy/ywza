import { LOCAL, useStore } from "../../api";

/** What this app may do with the live view of `dev`. Null permissions (this PC) allow everything. */
export function useAccess(dev: string) {
  const d = useStore((s) => s.devices[dev]);
  const p = d?.permissions;
  return {
    d,
    name: dev === LOCAL ? "This PC" : d?.name ?? dev,
    connected: !!d?.connected,
    canView: p?.view !== false,
    canControl: p?.control !== false,
    canSet: p?.settings !== false,
  };
}
