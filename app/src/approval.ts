import type { ApprovalMode } from "./api";

export const APPROVALS: { value: ApprovalMode; label: string }[] = [
  { value: "rules", label: "Follow rules" },
  { value: "auto", label: "Auto: run without asking" },
  { value: "always", label: "Ask before every action" },
];

export const approvalWarn = (m: ApprovalMode | undefined) => (m === "auto" ? "!border-warn text-warn" : "");
