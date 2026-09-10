import { useEffect, useMemo, useRef, useState } from "react";
import VaultModal from "../features/vault/components/VaultModal";
import { ModalCloseButton, useModalClose } from "../shared/components/Modal/Modal";
import VaultRecordDialog from "../features/vault/components/VaultRecordDialog";
import VaultRecordForm from "../features/vault/components/VaultRecordForm";
import VaultCardStack from "../features/vault/components/VaultCardStack";
import VaultListView from "../features/vault/components/VaultListView";
import VaultToolbar, {
  type VaultViewMode,
} from "../features/vault/components/VaultToolbar";
import { vaultClient } from "../features/vault/vaultClient";
import type { VaultRecordInput, VaultRecordSummary } from "../features/vault/types";

const LOAD_ERROR_MESSAGE = "KeyNest could not load your vault.";
const VAULT_VIEW_PREFERENCE_KEY = "keynest:vault-view";

function getInitialViewMode(): VaultViewMode {
  if (typeof window === "undefined") return "card";

  try {
    return window.localStorage.getItem(VAULT_VIEW_PREFERENCE_KEY) === "list"
      ? "list"
      : "card";
  } catch {
    return "card";
  }
}

function normalized(value: string) {
  return value.trim().toLocaleLowerCase();
}

type PasswordVaultPageProps = {
  favoriteRecordIds: ReadonlySet<string>;
  favoritesOnly?: boolean;
  onToggleFavorite(recordId: string): void;
};

export default function PasswordVaultPage({
  favoriteRecordIds,
  favoritesOnly = false,
  onToggleFavorite,
}: PasswordVaultPageProps) {
  const [records, setRecords] = useState<VaultRecordSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState("");
  const [search, setSearch] = useState("");
  const [tag, setTag] = useState("");
  const [viewMode, setViewMode] = useState<VaultViewMode>(getInitialViewMode);
  const [selectedRecordId, setSelectedRecordId] = useState<string | null>(null);
  const [isAdding, setIsAdding] = useState(false);
  const [isAddingPending, setIsAddingPending] = useState(false);
  const listRequestId = useRef(0);
  const addGeneration = useRef(0);
  const addNameRef = useRef<HTMLInputElement>(null);
  const addButtonRef = useRef<HTMLButtonElement>(null);

  async function loadRecords() {
    const requestId = ++listRequestId.current;
    setIsLoading(true);
    setLoadError("");
    try {
      const loaded = await vaultClient.listVaultRecords();
      if (listRequestId.current === requestId) setRecords(loaded);
    } catch {
      if (listRequestId.current === requestId) setLoadError(LOAD_ERROR_MESSAGE);
    } finally {
      if (listRequestId.current === requestId) setIsLoading(false);
    }
  }

  useEffect(() => {
    void loadRecords();
  }, []);

  useEffect(() => {
    try {
      window.localStorage.setItem(VAULT_VIEW_PREFERENCE_KEY, viewMode);
    } catch {
      // Storage availability should not prevent either view from working.
    }
  }, [viewMode]);

  const availableRecords = useMemo(
    () =>
      favoritesOnly
        ? records.filter((record) => favoriteRecordIds.has(record.id))
        : records,
    [favoriteRecordIds, favoritesOnly, records],
  );
  const tags = useMemo(
    () => [...new Set(availableRecords.flatMap((record) => record.tags))].sort(),
    [availableRecords],
  );
  const filteredRecords = useMemo(() => {
    const query = normalized(search);
    const selectedTag = normalized(tag);

    return availableRecords.filter((record) => {
      const searchValues = [record.name, record.username, record.website ?? "", ...record.tags];
      const matchesSearch =
        !query || searchValues.some((value) => normalized(value).includes(query));
      return (
        matchesSearch &&
        (!selectedTag || record.tags.some((value) => normalized(value) === selectedTag))
      );
    });
  }, [availableRecords, search, tag]);

  const addModal = useModalClose(() => {
    addGeneration.current += 1;
    setIsAdding(false);
  });

  async function createRecord(input: VaultRecordInput) {
    const generation = addGeneration.current;
    await vaultClient.createVaultRecord(input);
    if (addGeneration.current !== generation) {
      return;
    }
    await loadRecords();
    if (addGeneration.current !== generation) {
      return;
    }
    setIsAddingPending(false);
    addModal.close();
  }

  function openAddDialog() {
    addGeneration.current += 1;
    setIsAddingPending(false);
    setIsAdding(true);
  }

  function closeAddDialog() {
    if (isAddingPending) {
      return;
    }
    addModal.close();
  }

  return (
    <main className="password-vault-page">
      <header className="vault-page-heading">
        <div className="vault-heading-copy">
          <p className="eyebrow">
            {favoritesOnly ? "QUICK ACCESS" : "PRIVATE CREDENTIALS"}
          </p>
          <h1>{favoritesOnly ? "Favorites" : "Vault"}</h1>
          <p>
            {availableRecords.length}{" "}
            {availableRecords.length === 1 ? "credential" : "credentials"}
          </p>
        </div>
        <VaultToolbar
          search={search}
          tag={tag}
          tags={tags}
          viewMode={viewMode}
          addButtonRef={addButtonRef}
          onSearchChange={setSearch}
          onTagChange={setTag}
          onViewModeChange={setViewMode}
          onAddCredential={openAddDialog}
        />
      </header>

      {isLoading ? (
        <p className="vault-status" role="status">
          Loading credentials…
        </p>
      ) : null}
      {loadError ? (
        <section className="vault-load-error" aria-live="assertive">
          <p>{loadError}</p>
          <button className="secondary-button" type="button" onClick={() => void loadRecords()}>
            Retry
          </button>
        </section>
      ) : null}
      {!isLoading && !loadError && records.length === 0 ? (
        <section className="vault-empty-state">
          <h2>No credentials yet</h2>
          <p>Add your first credential to keep it protected in KeyNest.</p>
          <button className="primary-button" type="button" onClick={openAddDialog}>
            Add Credential
          </button>
        </section>
      ) : null}
      {!isLoading &&
      !loadError &&
      favoritesOnly &&
      records.length > 0 &&
      availableRecords.length === 0 ? (
        <section className="vault-empty-state">
          <h2>No favorite credentials</h2>
          <p>Use the Star on a credential to add it here.</p>
        </section>
      ) : null}
      {!isLoading &&
      !loadError &&
      availableRecords.length > 0 &&
      filteredRecords.length === 0 ? (
        <p className="vault-status">No matching credentials</p>
      ) : null}
      {!isLoading && !loadError && filteredRecords.length > 0 ? (
        viewMode === "card" ? (
          <VaultCardStack
            records={filteredRecords}
            favoriteRecordIds={favoriteRecordIds}
            onOpenRecord={setSelectedRecordId}
            onToggleFavorite={onToggleFavorite}
          />
        ) : (
          <VaultListView
            records={filteredRecords}
            favoriteRecordIds={favoriteRecordIds}
            onOpenRecord={setSelectedRecordId}
            onToggleFavorite={onToggleFavorite}
          />
        )
      ) : null}

      {isAdding ? (
        <VaultModal
          titleId="add-credential-title"
          className="add-credential-dialog"
          width={640}
          closing={addModal.closing}
          onExitComplete={addModal.finishClose}
          onRequestClose={closeAddDialog}
          isDismissDisabled={isAddingPending}
          initialFocusRef={addNameRef}
          fallbackFocusRef={addButtonRef}
        >
          <header className="vault-dialog-title">
            <div>
              <h2 id="add-credential-title">Add credential</h2>
            </div>
            <ModalCloseButton label="Close credential" onClick={closeAddDialog} disabled={isAddingPending || addModal.closing} />
          </header>
          <VaultRecordForm
            onSubmit={createRecord}
            onCancel={closeAddDialog}
            onPendingChange={setIsAddingPending}
            initialFocusRef={addNameRef}
          />
        </VaultModal>
      ) : null}
      {selectedRecordId ? (
        <VaultRecordDialog
          key={selectedRecordId}
          recordId={selectedRecordId}
          isFavorite={favoriteRecordIds.has(selectedRecordId)}
          onClose={() => setSelectedRecordId(null)}
          onChanged={loadRecords}
          onToggleFavorite={() => onToggleFavorite(selectedRecordId)}
          fallbackFocusRef={addButtonRef}
        />
      ) : null}
    </main>
  );
}
