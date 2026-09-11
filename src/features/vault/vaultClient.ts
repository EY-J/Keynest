import { invoke } from "@tauri-apps/api/core";
import { publicError } from "../../shared/security/publicErrors";
import {
  VaultClientError,
  type Credential,
  type CredentialInput,
  type CredentialSummary,
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
  listCredentials: () =>
    invokeVault<CredentialSummary[]>("list_vault_records"),
  createCredential: (input: CredentialInput) =>
    invokeVault<CredentialSummary>("create_vault_record", { input }),
  getCredential: (id: string) =>
    invokeVault<Credential>("get_vault_record", { id }),
  getCredentialSummary: (id: string) =>
    invokeVault<CredentialSummary>("get_vault_record_summary", { id }),
  updateCredential: (id: string, input: CredentialInput) =>
    invokeVault<CredentialSummary>("update_vault_record", { id, input }),
  deleteCredential: (id: string) =>
    invokeVault<void>("delete_vault_record", { id }),
  copyCredentialPassword: (id: string) =>
    invokeVault<void>("copy_vault_password", { id }),
  copyCredentialUsername: (id: string) =>
    invokeVault<void>("copy_vault_username", { id }),
};
