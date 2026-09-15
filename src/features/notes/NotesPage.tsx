import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import NotesToolbar from "./components/NotesToolbar";
import NotesList from "./components/NotesList";
import NoteEditorPanel from "./components/NoteEditorPanel";
import DeleteNoteDialog from "./components/DeleteNoteDialog";
import { notesClient } from "./notesClient";
import { filterNotes } from "./noteUtils";
import type { Note, NoteInput } from "./types";
import { useToast } from "../../components/ui/Toast/ToastProvider";

function duplicateTitle(title: string, notes: Note[]) {
  const sourceTitle = title.trim() || "Untitled note";
  const copyTitle = `${sourceTitle} copy`;
  const existingTitles = new Set(
    notes.map((note) => note.title.trim().toLocaleLowerCase()),
  );

  if (!existingTitles.has(copyTitle.toLocaleLowerCase())) return copyTitle;

  let copyNumber = 2;
  while (existingTitles.has(`${copyTitle} ${copyNumber}`.toLocaleLowerCase())) {
    copyNumber += 1;
  }
  return `${copyTitle} ${copyNumber}`;
}

export default function NotesPage() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [search, setSearch] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [deletingNote, setDeletingNote] = useState<Note | null>(null);
  const [deletePending, setDeletePending] = useState(false);
  const [deleteError, setDeleteError] = useState("");
  const addButtonRef = useRef<HTMLButtonElement>(null);
  const { showToast } = useToast();

  const loadNotes = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      setNotes(await notesClient.list());
    } catch {
      setError("KeyNest could not load your notes.");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void loadNotes(); }, [loadNotes]);

  const visibleNotes = useMemo(() => filterNotes(notes, search), [notes, search]);
  const selectedNote = notes.find((note) => note.id === selectedId) ?? null;

  const replaceNote = useCallback((saved: Note) => {
    setNotes((current) => current.map((note) => note.id === saved.id ? saved : note));
  }, []);

  async function createNote() {
    if (creating) return;
    setCreating(true);
    setError("");
    try {
      const created = await notesClient.create({ title: "Untitled note", content: "", tags: [], favorite: false });
      setNotes((current) => [created, ...current]);
      setSelectedId(created.id);
    } catch {
      setError("KeyNest could not create a note.");
    } finally {
      setCreating(false);
    }
  }

  async function toggleFavorite(note: Note) {
    const input: NoteInput = {
      title: note.title,
      content: note.content,
      tags: note.tags,
      favorite: !note.favorite,
    };
    try {
      replaceNote(await notesClient.update(note.id, input));
    } catch {
      setError("KeyNest could not update this note.");
    }
  }

  async function duplicateNote(note: Note) {
    if (creating) return;
    setCreating(true);
    setError("");
    try {
      const created = await notesClient.create({
        title: duplicateTitle(note.title, notes),
        content: note.content,
        tags: [...note.tags],
        favorite: false,
      });
      setNotes((current) => [created, ...current]);
      setSelectedId(created.id);
    } catch {
      setError("KeyNest could not duplicate this note.");
    } finally {
      setCreating(false);
    }
  }

  async function confirmDelete() {
    if (!deletingNote || deletePending) return false;
    setDeletePending(true);
    setDeleteError("");
    try {
      await notesClient.delete(deletingNote.id);
      setNotes((current) => current.filter((note) => note.id !== deletingNote.id));
      if (selectedId === deletingNote.id) {
        setSelectedId(null);
      }
      showToast({ type: "success", message: "Moved to Recently Deleted" });
      return true;
    } catch {
      setDeleteError("KeyNest could not delete this note.");
      return false;
    } finally {
      setDeletePending(false);
    }
  }

  return (
    <main className="password-vault-page notes-page">
      {loading ? <p className="vault-status" role="status">Loading notes…</p> : null}
      {error ? (
        <section className="vault-load-error" aria-live="assertive">
          <p>{error}</p>
          <button className="secondary-button" type="button" onClick={() => void loadNotes()}>Retry</button>
        </section>
      ) : null}
      {!loading && !error ? (
        <div className="notes-workspace">
          <aside className="notes-list-panel" aria-label="Notes list panel">
            <NotesToolbar
              search={search}
              creating={creating}
              addButtonRef={addButtonRef}
              onSearchChange={setSearch}
              onCreate={() => void createNote()}
            />
            <NotesList
              notes={visibleNotes}
              selectedId={selectedId}
              hasSearch={Boolean(search.trim())}
              onSelect={setSelectedId}
            />
          </aside>
          <NoteEditorPanel
            note={selectedNote}
            creating={creating}
            onCreate={() => void createNote()}
            onSaved={replaceNote}
            onDelete={(note) => { setDeleteError(""); setDeletingNote(note); }}
            onDuplicate={(note) => void duplicateNote(note)}
            onToggleFavorite={(note) => void toggleFavorite(note)}
          />
        </div>
      ) : null}

      {deletingNote ? (
        <DeleteNoteDialog
          note={deletingNote}
          pending={deletePending}
          error={deleteError}
          fallbackFocusRef={addButtonRef}
          onCancel={() => { if (!deletePending) setDeletingNote(null); }}
          onConfirm={confirmDelete}
        />
      ) : null}
    </main>
  );
}
