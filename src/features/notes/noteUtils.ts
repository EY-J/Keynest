import type { Note } from "./types";

function normalized(value: string) {
  return value.trim().toLocaleLowerCase();
}

export function filterNotes(notes: Note[], search: string) {
  const query = normalized(search);
  if (!query) return notes;
  return notes.filter((note) =>
    [note.title, note.content].some((value) => normalized(value).includes(query)),
  );
}
