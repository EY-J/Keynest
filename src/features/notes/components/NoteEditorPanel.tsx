import { useEffect, useRef, useState } from "react";
import {
  Copy,
  FolderPlus,
  MoreHorizontal,
  Plus,
  Star,
  Trash2,
} from "lucide-react";
import { EyeIcon, type EyeIconHandle } from "../../../components/ui/eye";
import {
  SquarePenIcon,
  type SquarePenIconHandle,
} from "../../../components/ui/square-pen";
import type { Note, NoteInput } from "../types";
import { notesClient } from "../notesClient";

type NoteEditorPanelProps = {
  note: Note | null;
  creating: boolean;
  onCreate(): void;
  onSaved(note: Note): void;
  onDelete(note: Note): void;
  onDuplicate(note: Note): void;
  onToggleFavorite(note: Note): void;
};

type EditorMode = "edit" | "preview";

export default function NoteEditorPanel({
  note,
  creating,
  onCreate,
  onSaved,
  onDelete,
  onDuplicate,
  onToggleFavorite,
}: NoteEditorPanelProps) {
  const [draft, setDraft] = useState<NoteInput | null>(null);
  const [dirty, setDirty] = useState(false);
  const [status, setStatus] = useState<"" | "Saving..." | "Saved" | "Save failed" | "Title required">("");
  const [menuOpen, setMenuOpen] = useState(false);
  const [editorMode, setEditorMode] = useState<EditorMode>("edit");
  const revisionRef = useRef(0);
  const saveQueueRef = useRef<Promise<void>>(Promise.resolve());
  const activeNoteIdRef = useRef<string | null>(null);
  const draftRef = useRef<NoteInput | null>(null);
  const dirtyRef = useRef(false);
  const actionsRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuTriggerRef = useRef<HTMLButtonElement>(null);
  const editIconRef = useRef<SquarePenIconHandle>(null);
  const previewIconRef = useRef<EyeIconHandle>(null);
  draftRef.current = draft;
  dirtyRef.current = dirty;

  useEffect(() => {
    const noteId = note?.id ?? null;
    activeNoteIdRef.current = noteId;
    setDraft(note ? { title: note.title, content: note.content, tags: note.tags, favorite: note.favorite } : null);
    setDirty(false);
    setStatus("");
    revisionRef.current += 1;
    return () => {
      const pending = draftRef.current;
      if (!noteId || !dirtyRef.current || !pending?.title.trim()) return;
      const snapshot = { ...pending, title: pending.title.trim(), tags: [...pending.tags] };
      dirtyRef.current = false;
      saveQueueRef.current = saveQueueRef.current
        .catch(() => undefined)
        .then(async () => onSaved(await notesClient.update(noteId, snapshot)))
        .catch(() => undefined);
    };
  }, [note?.id]);

  useEffect(() => {
    if (note) setDraft((current) => current ? { ...current, favorite: note.favorite } : current);
  }, [note?.favorite]);

  useEffect(() => {
    setMenuOpen(false);
    setEditorMode("edit");
  }, [note?.id]);

  useEffect(() => {
    if (!menuOpen) return;

    const firstMenuItem = menuRef.current?.querySelector<HTMLButtonElement>(
      '[role="menuitem"]:not(:disabled)',
    );
    firstMenuItem?.focus({ preventScroll: true });

    function closeOnOutsidePointer(event: PointerEvent) {
      if (
        event.target instanceof Node
        && !actionsRef.current?.contains(event.target)
      ) {
        setMenuOpen(false);
      }
    }

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      event.preventDefault();
      setMenuOpen(false);
      menuTriggerRef.current?.focus({ preventScroll: true });
    }

    document.addEventListener("pointerdown", closeOnOutsidePointer);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOnOutsidePointer);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [menuOpen]);

  useEffect(() => {
    if (!note || !draft || !dirty) return;
    if (!draft.title.trim()) {
      setStatus("Title required");
      return;
    }
    const noteId = note.id;
    const revision = revisionRef.current;
    const snapshot = { ...draft, title: draft.title.trim(), tags: [...draft.tags] };
    const timer = window.setTimeout(() => {
      setStatus("Saving...");
      saveQueueRef.current = saveQueueRef.current
        .catch(() => undefined)
        .then(async () => {
          const saved = await notesClient.update(noteId, snapshot);
          if (activeNoteIdRef.current !== noteId) return;
          onSaved(saved);
          if (revisionRef.current === revision) {
            setDirty(false);
            setStatus("Saved");
          }
        })
        .catch(() => {
          if (activeNoteIdRef.current === noteId && revisionRef.current === revision) setStatus("Save failed");
        });
    }, 650);
    return () => window.clearTimeout(timer);
  }, [dirty, draft, note, onSaved]);

  if (!note || !draft) {
    return (
      <section className="notes-detail-panel notes-no-selection">
        <h2>No note selected</h2>
        <p>Select a note or create a new one.</p>
        <button
          className="primary-button notes-empty-add"
          type="button"
          aria-label="New note"
          title="New note"
          onClick={onCreate}
          disabled={creating}
        >
          <Plus size={18} aria-hidden="true" />
        </button>
      </section>
    );
  }

  function change(patch: Partial<NoteInput>) {
    setDraft((current) => current ? { ...current, ...patch } : current);
    revisionRef.current += 1;
    setDirty(true);
    setStatus("Saving...");
  }

  function toggleFavorite() {
    if (!note || !draft) return;
    const nextFavorite = !draft.favorite;
    setDraft((current) => current ? { ...current, favorite: nextFavorite } : current);
    onToggleFavorite({ ...note, ...draft, favorite: nextFavorite });
  }

  function selectFavoriteAction() {
    toggleFavorite();
    setMenuOpen(false);
  }

  function selectDuplicateAction() {
    if (!note || !draft) return;
    setMenuOpen(false);
    onDuplicate({ ...note, ...draft });
  }

  function selectDeleteAction() {
    if (!note) return;
    setMenuOpen(false);
    onDelete(note);
  }

  function moveMenuFocus(event: React.KeyboardEvent<HTMLDivElement>) {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;

    const menuItems = Array.from(
      event.currentTarget.querySelectorAll<HTMLButtonElement>(
        '[role="menuitem"]:not(:disabled)',
      ),
    );
    if (menuItems.length === 0) return;

    event.preventDefault();
    const currentIndex = menuItems.indexOf(document.activeElement as HTMLButtonElement);
    const nextIndex = event.key === "Home"
      ? 0
      : event.key === "End"
        ? menuItems.length - 1
        : event.key === "ArrowDown"
          ? (currentIndex + 1 + menuItems.length) % menuItems.length
          : (currentIndex - 1 + menuItems.length) % menuItems.length;
    menuItems[nextIndex]?.focus({ preventScroll: true });
  }

  function selectEditorMode(mode: EditorMode) {
    animateModeIcon(mode, true);
    if (mode === editorMode) return;
    setEditorMode(mode);
  }

  function animateModeIcon(mode: EditorMode, start: boolean) {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const icon = mode === "edit" ? editIconRef.current : previewIconRef.current;
    if (start) icon?.startAnimation();
    else icon?.stopAnimation();
  }

  return (
    <section className="notes-detail-panel notes-editor-panel">
      <header className="notes-editor-header">
        {editorMode === "edit" ? (
          <input
            className="notes-title-input"
            value={draft.title}
            maxLength={200}
            aria-label="Note title"
            placeholder="Untitled note"
            onChange={(event) => change({ title: event.target.value })}
          />
        ) : (
          <h2 className="notes-preview-title">
            {draft.title.trim() || "Untitled note"}
          </h2>
        )}
        <span className={`notes-save-status${status === "Save failed" || status === "Title required" ? " error" : ""}`} role="status">{status}</span>
        <div className="vault-view-toggle notes-view-toggle" role="group" aria-label="Note mode">
          <button
            className={`notes-mode-edit${editorMode === "edit" ? " active" : ""}`}
            type="button"
            aria-label="Edit note"
            aria-pressed={editorMode === "edit"}
            title="Edit"
            onMouseEnter={() => animateModeIcon("edit", true)}
            onMouseLeave={() => animateModeIcon("edit", false)}
            onFocus={() => animateModeIcon("edit", true)}
            onBlur={() => animateModeIcon("edit", false)}
            onClick={() => selectEditorMode("edit")}
          >
            <SquarePenIcon
              ref={editIconRef}
              size={18}
              className="notes-mode-icon"
              aria-hidden="true"
            />
          </button>
          <button
            className={`notes-mode-preview${editorMode === "preview" ? " active" : ""}`}
            type="button"
            aria-label="Preview note"
            aria-pressed={editorMode === "preview"}
            title="Preview"
            onMouseEnter={() => animateModeIcon("preview", true)}
            onMouseLeave={() => animateModeIcon("preview", false)}
            onFocus={() => animateModeIcon("preview", true)}
            onBlur={() => animateModeIcon("preview", false)}
            onClick={() => selectEditorMode("preview")}
          >
            <EyeIcon
              ref={previewIconRef}
              size={18}
              className="notes-mode-icon"
              aria-hidden="true"
            />
          </button>
        </div>
        <div className="notes-editor-actions" ref={actionsRef}>
          <button
            ref={menuTriggerRef}
            className="notes-action-menu-trigger notes-editor-action"
            type="button"
            aria-label="Note actions"
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            aria-controls={menuOpen ? "note-actions-menu" : undefined}
            title="Note actions"
            onClick={() => setMenuOpen((current) => !current)}
          >
            <MoreHorizontal size={18} aria-hidden="true" />
          </button>

          {menuOpen ? (
            <div
              ref={menuRef}
              id="note-actions-menu"
              className="notes-action-menu"
              role="menu"
              aria-label="Note actions"
              onKeyDown={moveMenuFocus}
            >
              <button type="button" role="menuitem" onClick={selectFavoriteAction}>
                <Star
                  size={15}
                  fill={draft.favorite ? "currentColor" : "none"}
                  aria-hidden="true"
                />
                <span>{draft.favorite ? "Remove from favorites" : "Add to favorites"}</span>
              </button>
              <button type="button" role="menuitem" disabled>
                <FolderPlus size={15} aria-hidden="true" />
                <span>Add to folder</span>
                <small>Soon</small>
              </button>
              <button type="button" role="menuitem" onClick={selectDuplicateAction}>
                <Copy size={15} aria-hidden="true" />
                <span>Duplicate</span>
              </button>
              <div className="notes-action-menu-separator" role="separator" />
              <button
                className="danger"
                type="button"
                role="menuitem"
                onClick={selectDeleteAction}
              >
                <Trash2 size={15} aria-hidden="true" />
                <span>Delete</span>
              </button>
            </div>
          ) : null}
        </div>
      </header>
      {editorMode === "edit" ? (
        <textarea
          className="notes-content-input"
          value={draft.content}
          maxLength={1_000_000}
          aria-label="Note content"
          placeholder="Start writing..."
          onChange={(event) => change({ content: event.target.value })}
        />
      ) : (
        <div
          className="notes-preview-content"
          role="region"
          aria-label="Note preview"
          tabIndex={0}
        >
          {draft.content}
        </div>
      )}
    </section>
  );
}
