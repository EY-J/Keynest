import { invoke } from "@tauri-apps/api/core";
import { publicError } from "../../shared/security/publicErrors";
import {
  AuthClientError,
  type AuthStatus,
  type RecoveryKeyResult,
  type RecoveryStatus,
} from "./types";

type InvokeArguments = Record<string, unknown>;

async function invokeAuth<T>(
  command: string,
  argumentsValue?: InvokeArguments,
): Promise<T> {
  try {
    if (argumentsValue === undefined) {
      return await invoke<T>(command);
    }
    return await invoke<T>(command, argumentsValue);
  } catch (error) {
    throw normalizeAuthError(error);
  }
}

function normalizeAuthError(error: unknown): AuthClientError {
  const { code, message, retryAfterMs } = publicError(error);
  return new AuthClientError(code, message, retryAfterMs);
}

export const authClient = {
  getStatus: () => invokeAuth<AuthStatus>("get_auth_status"),
  createMasterPassword: (password: string) =>
    invokeAuth<RecoveryKeyResult>("create_master_password", { password }),
  completeRecoveryKeyDisplay: () =>
    invokeAuth<AuthStatus>("complete_recovery_key_display"),
  getRecoveryStatus: () =>
    invokeAuth<RecoveryStatus>("get_recovery_status"),
  recoverMasterPassword: (recoveryKey: string, newPassword: string) =>
    invokeAuth<RecoveryKeyResult>("recover_master_password", {
      recoveryKey,
      newPassword,
    }),
  regenerateRecoveryKey: (currentPassword: string) =>
    invokeAuth<RecoveryKeyResult>("regenerate_recovery_key", {
      currentPassword,
    }),
  copyRecoveryKey: (recoveryKey: string) =>
    invokeAuth<void>("copy_recovery_key", { recoveryKey }),
  unlock: (password: string) =>
    invokeAuth<AuthStatus>("unlock", { password }),
  lock: () => invokeAuth<AuthStatus>("lock"),
  changeMasterPassword: (currentPassword: string, newPassword: string) =>
    invokeAuth<AuthStatus>("change_master_password", {
      currentPassword,
      newPassword,
    }),
  resetKeynest: (confirmation: string) =>
    invokeAuth<AuthStatus>("reset_keynest", { confirmation }),
  resetKeynestAuthenticated: (currentPassword: string, confirmation: string) =>
    invokeAuth<AuthStatus>("reset_keynest_authenticated", {
      currentPassword,
      confirmation,
    }),
};
