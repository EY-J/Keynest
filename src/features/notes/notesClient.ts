import { invoke } from "@tauri-apps/api/core";
import { publicError } from "../../shared/security/publicErrors";
import type { Note, NoteInput } from "./types";

async function invokeNotes<T>(command: string, argumentsValue?: Record<string, unknown>): Promise<T> {
  try {
    return argumentsValue === undefined
      ? await invoke<T>(command)
      : await invoke<T>(command, argumentsValue);
  } catch (error) {
    const { message } = publicError(error);
    throw new Error(message);
  }
}

export const notesClient = {
  list: () => invokeNotes<Note[]>("list_notes"),
  create: (input: NoteInput) => invokeNotes<Note>("create_note", { input }),
  update: (id: string, input: NoteInput) => invokeNotes<Note>("update_note", { id, input }),
  delete: (id: string) => invokeNotes<void>("delete_note", { id }),
};
