import { invoke } from "@tauri-apps/api/core";
import {
  VaultClientError,
  type VaultRecord,
  type VaultRecordInput,
  type VaultRecordSummary,
} from "./types";

type InvokeArguments = Record<string, unknown>;

const UNKNOWN_VAULT_ERROR_MESSAGE =
  "KeyNest could not complete the vault request.";

async function invokeVault<T>(
  command: string,
  argumentsValue?: InvokeArguments,
): Promise<T> {
  try {
    if (argumentsValue === undefined) {
      return await invoke<T>(command);
    }
    return await invoke<T>(command, argumentsValue);
  } catch (error) {
    throw normalizeVaultError(error);
  }
}

function normalizeVaultError(error: unknown): VaultClientError {
  if (isStructuredVaultError(error)) {
    return new VaultClientError(error.code, error.message);
  }

  return new VaultClientError("unknown-error", UNKNOWN_VAULT_ERROR_MESSAGE);
}

function isStructuredVaultError(
  value: unknown,
): value is { code: string; message: string } {
  return (
    isRecord(value) &&
    !(value instanceof Error) &&
    typeof value.code === "string" &&
    typeof value.message === "string"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export const vaultClient = {
  listVaultRecords: () =>
    invokeVault<VaultRecordSummary[]>("list_vault_records"),
  createVaultRecord: (input: VaultRecordInput) =>
    invokeVault<VaultRecordSummary>("create_vault_record", { input }),
  getVaultRecord: (id: string) =>
    invokeVault<VaultRecord>("get_vault_record", { id }),
  updateVaultRecord: (id: string, input: VaultRecordInput) =>
    invokeVault<VaultRecordSummary>("update_vault_record", { id, input }),
  deleteVaultRecord: (id: string) =>
    invokeVault<void>("delete_vault_record", { id }),
  copyVaultPassword: (id: string) =>
    invokeVault<void>("copy_vault_password", { id }),
};
