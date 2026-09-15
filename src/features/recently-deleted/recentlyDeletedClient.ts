import { invoke } from "@tauri-apps/api/core";
import { publicError } from "../../shared/security/publicErrors";
import type { DeletedItem, DeletedItemType } from "./types";

async function invokeDeleted<T>(command: string, argumentsValue?: Record<string, unknown>) {
  try {
    return argumentsValue === undefined
      ? await invoke<T>(command)
      : await invoke<T>(command, argumentsValue);
  } catch (error) {
    throw new Error(publicError(error).message);
  }
}

export const recentlyDeletedClient = {
  list: () => invokeDeleted<DeletedItem[]>("list_deleted_items"),
  restore: (id: string, itemType: DeletedItemType) =>
    invokeDeleted<void>("restore_deleted_item", { id, itemType }),
  permanentlyDelete: (id: string, itemType: DeletedItemType) =>
    invokeDeleted<void>("permanently_delete_item", { id, itemType }),
  empty: () => invokeDeleted<void>("empty_recently_deleted"),
};
