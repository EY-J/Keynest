import { invoke } from "@tauri-apps/api/core";
import { publicError } from "../../shared/security/publicErrors";

export type PendingHostApproval = {
  approvalId: string;
  requestedHost: string;
};

export type HostApprovalCandidate = {
  credentialId: string;
  name: string;
  username: string;
  website: string | null;
};

export type HostApprovalCompleted = {
  credentialName: string;
  requestedHost: string;
  changed: boolean;
};

export class HostApprovalClientError extends Error {
  constructor(public readonly code: string, message: string) {
    super(message);
  }
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return args ? await invoke<T>(command, args) : await invoke<T>(command);
  } catch (error) {
    const normalized = publicError(error);
    throw new HostApprovalClientError(normalized.code, normalized.message);
  }
}

export const hostApprovalClient = {
  pending: () => call<PendingHostApproval | null>("get_pending_host_approval"),
  candidates: (approvalId: string) =>
    call<HostApprovalCandidate[]>("list_host_approval_candidates", { approvalId }),
  approve: (approvalId: string, credentialId: string) =>
    call<HostApprovalCompleted>("approve_login_host", { approvalId, credentialId }),
  cancel: (approvalId: string) =>
    call<void>("cancel_host_approval", { approvalId }),
};
