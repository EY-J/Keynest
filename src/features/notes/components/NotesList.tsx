import type { Note } from "../types";

const relativeTime = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
const shortDate = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });

function formatUpdated(timestamp: number) {
  const elapsed = timestamp - Date.now();
  const minutes = Math.round(elapsed / 60_000);
  if (Math.abs(minutes) < 60) return relativeTime.format(minutes, "minute");
  const hours = Math.round(elapsed / 3_600_000);
  if (Math.abs(hours) < 24) return relativeTime.format(hours, "hour");
  const days = Math.round(elapsed / 86_400_000);
  if (Math.abs(days) < 7) return relativeTime.format(days, "day");
  return shortDate.format(new Date(timestamp));
}

function preview(content: string) {
  return content.replace(/\s+/g, " ").trim() || "No content yet";
}

type NotesListProps = {
  notes: Note[];
  selectedId: string | null;
  hasSearch: boolean;
  onSelect(id: string): void;
};

export default function NotesList({ notes, selectedId, hasSearch, onSelect }: NotesListProps) {
  if (notes.length === 0) {
    return <p className="notes-list-empty">{hasSearch ? "No matching notes" : "No notes yet"}</p>;
  }

  return (
    <div className="notes-list" role="list" aria-label="Notes">
      {notes.map((note) => (
        <button
          className={`notes-list-item${selectedId === note.id ? " selected" : ""}`}
          key={note.id}
          type="button"
          role="listitem"
          aria-current={selectedId === note.id ? "true" : undefined}
          onClick={() => onSelect(note.id)}
        >
          <strong>{note.title}</strong>
          <span className="notes-list-preview">{preview(note.content)}</span>
          <time dateTime={new Date(note.updatedAtMs).toISOString()}>{formatUpdated(note.updatedAtMs)}</time>
        </button>
      ))}
    </div>
  );
}
