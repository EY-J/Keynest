import { useEffect, useRef } from "react";
import { Plus, Search } from "lucide-react";

type NotesToolbarProps = {
  search: string;
  creating: boolean;
  addButtonRef: React.RefObject<HTMLButtonElement | null>;
  onSearchChange(value: string): void;
  onCreate(): void;
};

export default function NotesToolbar({
  search,
  creating,
  addButtonRef,
  onSearchChange,
  onCreate,
}: NotesToolbarProps) {
  const searchInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    function focusSearch(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLocaleLowerCase() === "k") {
        event.preventDefault();
        searchInputRef.current?.focus();
      }
    }
    window.addEventListener("keydown", focusSearch);
    return () => window.removeEventListener("keydown", focusSearch);
  }, []);

  return (
    <div className="notes-sidebar-controls">
      <div className="notes-sidebar-heading">
        <h1>Notes</h1>
        <button
          ref={addButtonRef}
          className="primary-button notes-add-button"
          type="button"
          aria-label="New note"
          title="New note"
          onClick={onCreate}
          disabled={creating}
        >
          <Plus size={18} strokeWidth={2.4} aria-hidden="true" />
        </button>
      </div>
      <label className="vault-search-control">
        <span className="sr-only">Search notes</span>
        <Search size={15} aria-hidden="true" />
        <input
          ref={searchInputRef}
          type="search"
          value={search}
          placeholder="Search notes..."
          onChange={(event) => onSearchChange(event.target.value)}
        />
        <kbd>Ctrl K</kbd>
      </label>
    </div>
  );
}
