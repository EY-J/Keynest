import { invoke } from "@tauri-apps/api/core";
import { AuthClientError, type AuthStatus } from "./types";

type InvokeArguments = Record<string, unknown>;

async function invokeAuth(
  command: string,
  argumentsValue?: InvokeArguments,
): Promise<AuthStatus> {
  try {
    if (argumentsValue === undefined) {
      return await invoke<AuthStatus>(command);
    }
    return await invoke<AuthStatus>(command, argumentsValue);
  } catch (error) {
    throw normalizeAuthError(error);
  }
}

function normalizeAuthError(error: unknown): AuthClientError {
  if (isRecord(error)) {
    const code = typeof error.code === "string" ? error.code : "unknown-error";
    const message =
      typeof error.message === "string"
        ? error.message
        : "KeyNest could not complete the security request.";
    const retryAfterMs =
      typeof error.retryAfterMs === "number" ? error.retryAfterMs : undefined;
    return new AuthClientError(code, message, retryAfterMs);
  }

  return new AuthClientError(
    "unknown-error",
    "KeyNest could not complete the security request.",
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export const authClient = {
  getStatus: () => invokeAuth("get_auth_status"),
  createMasterPassword: (password: string) =>
    invokeAuth("create_master_password", { password }),
  unlock: (password: string) => invokeAuth("unlock", { password }),
  lock: () => invokeAuth("lock"),
  resetKeynest: (confirmation: string) =>
    invokeAuth("reset_keynest", { confirmation }),
};
