import { invoke } from "@tauri-apps/api/core";
import { publicError } from "../../shared/security/publicErrors";
import {
  VaultClientError,
  type VaultRecord,
  type VaultRecordInput,
  type VaultRecordSummary,
} from "./types";

type InvokeArguments = Record<string, unknown>;

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
  const { code, message } = publicError(error);
  return new VaultClientError(code, message);
}

export const vaultClient = {
  listVaultRecords: () =>
    invokeVault<VaultRecordSummary[]>("list_vault_records"),
  createVaultRecord: (input: VaultRecordInput) =>
    invokeVault<VaultRecordSummary>("create_vault_record", { input }),
  getVaultRecord: (id: string) =>
    invokeVault<VaultRecord>("get_vault_record", { id }),
  getVaultRecordSummary: (id: string) =>
    invokeVault<VaultRecordSummary>("get_vault_record_summary", { id }),
  updateVaultRecord: (id: string, input: VaultRecordInput) =>
    invokeVault<VaultRecordSummary>("update_vault_record", { id, input }),
  deleteVaultRecord: (id: string) =>
    invokeVault<void>("delete_vault_record", { id }),
  copyVaultPassword: (id: string) =>
    invokeVault<void>("copy_vault_password", { id }),
  copyVaultUsername: (id: string) =>
    invokeVault<void>("copy_vault_username", { id }),
};
