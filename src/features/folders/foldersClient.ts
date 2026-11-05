import { invoke } from "@tauri-apps/api/core";
import { publicError } from "../../shared/security/publicErrors";

export type Folder = {
  id: string;
  name: string;
  createdAtMs: number;
  updatedAtMs: number;
};
export type FolderItem = {
  kind: "credential" | "note";
  itemId: string;
  folderId: string;
};

async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(publicError(error).message);
  }
}

export const foldersClient = {
  list: () => call<Folder[]>("list_folders"),
  create: (name: string) => call<Folder>("create_folder", { name }),
  rename: (id: string, name: string) =>
    call<Folder>("rename_folder", { id, name }),
  delete: (id: string) => call<void>("delete_folder", { id }),
  items: () => call<FolderItem[]>("list_folder_items"),
  contents: (folderId: string) =>
    call<FolderItem[]>("get_folder_contents", { folderId }),
  assignCredential: (id: string, folderId: string | null) =>
    call<void>("assign_credential_folder", { id, folderId }),
  assignNote: (id: string, folderId: string | null) =>
    call<void>("assign_note_folder", { id, folderId }),
};
