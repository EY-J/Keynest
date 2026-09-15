import { useEffect, useRef } from "react";
import { Layers3, List, Plus, Search, Tag } from "lucide-react";
import Select from "../../../components/ui/Select";

export type VaultViewMode = "card" | "list";

type VaultToolbarProps = {
  search: string;
  tag: string;
  tags: string[];
  viewMode: VaultViewMode;
  addButtonRef: React.RefObject<HTMLButtonElement | null>;
  onSearchChange(value: string): void;
  onTagChange(value: string): void;
  onViewModeChange(mode: VaultViewMode): void;
  onAddCredential(): void;
};

export default function VaultToolbar({
  search,
  tag,
  tags,
  viewMode,
  addButtonRef,
  onSearchChange,
  onTagChange,
  onViewModeChange,
  onAddCredential,
}: VaultToolbarProps) {
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
    <div className="vault-toolbar" role="group" aria-label="Vault controls">
      <label className="vault-search-control">
        <span className="sr-only">Search credentials</span>
        <Search size={15} aria-hidden="true" />
        <input
          ref={searchInputRef}
          type="search"
          value={search}
          placeholder="Search credentials..."
          onChange={(event) => onSearchChange(event.target.value)}
        />
        <kbd>Ctrl K</kbd>
      </label>

      <Select
        className="vault-filter-control"
        menuClassName="vault-tag-select-menu"
        value={tag}
        options={tags.map((option) => ({ value: option, label: option }))}
        onChange={onTagChange}
        onClear={() => onTagChange("")}
        placeholder="Tag"
        ariaLabel="Filter by tag"
        icon={<Tag size={15} />}
      />

      <div className="vault-view-toggle" role="group" aria-label="Credential view">
        <button
          className={viewMode === "card" ? "active" : ""}
          type="button"
          title="Card Stack View"
          aria-label="Card Stack View"
          aria-pressed={viewMode === "card"}
          onClick={() => onViewModeChange("card")}
        >
          <Layers3 size={16} aria-hidden="true" />
        </button>
        <button
          className={viewMode === "list" ? "active" : ""}
          type="button"
          title="List View"
          aria-label="List View"
          aria-pressed={viewMode === "list"}
          onClick={() => onViewModeChange("list")}
        >
          <List size={17} aria-hidden="true" />
        </button>
      </div>

      <button
        ref={addButtonRef}
        data-vault-modal-fallback
        className="primary-button compact-button vault-add-button"
        type="button"
        onClick={onAddCredential}
      >
        <Plus size={15} strokeWidth={2.4} aria-hidden="true" />
        New
      </button>
    </div>
  );
}
