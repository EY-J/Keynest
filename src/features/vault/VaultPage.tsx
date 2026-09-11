import { useEffect, useMemo, useRef, useState } from "react";
import VaultModal from "./components/VaultModal";
import { ModalCloseButton, useModalClose } from "../../components/ui/Modal/Modal";
import CredentialDetailsModal from "./components/CredentialDetailsModal";
import CredentialForm from "./components/CredentialForm";
import CredentialCardStack from "./components/CredentialCardStack";
import CredentialList from "./components/CredentialList";
import VaultToolbar, {
  type VaultViewMode,
} from "./components/VaultToolbar";
import { vaultClient } from "./vaultClient";
import type { CredentialInput, CredentialSummary } from "./types";

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

function normalizeSearchText(value: string) {
  return value.trim().toLocaleLowerCase();
}

type VaultPageProps = {
  favoriteCredentialIds: ReadonlySet<string>;
  favoritesOnly?: boolean;
  onToggleFavorite(credentialId: string): void;
};

export default function VaultPage({
  favoriteCredentialIds,
  favoritesOnly = false,
  onToggleFavorite,
}: VaultPageProps) {
  const [credentials, setCredentials] = useState<CredentialSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState("");
  const [search, setSearch] = useState("");
  const [tag, setTag] = useState("");
  const [viewMode, setViewMode] = useState<VaultViewMode>(getInitialViewMode);
  const [selectedCredentialId, setSelectedCredentialId] = useState<string | null>(null);
  const [isAdding, setIsAdding] = useState(false);
  const [isAddingPending, setIsAddingPending] = useState(false);
  const loadRequestId = useRef(0);
  const addDialogGeneration = useRef(0);
  const addNameRef = useRef<HTMLInputElement>(null);
  const addButtonRef = useRef<HTMLButtonElement>(null);

  async function loadRecords() {
    const requestId = ++loadRequestId.current;
    setIsLoading(true);
    setLoadError("");
    try {
      const loaded = await vaultClient.listCredentials();
      if (loadRequestId.current === requestId) setCredentials(loaded);
    } catch {
      if (loadRequestId.current === requestId) setLoadError(LOAD_ERROR_MESSAGE);
    } finally {
      if (loadRequestId.current === requestId) setIsLoading(false);
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

  const availableCredentials = useMemo(
    () =>
      favoritesOnly
        ? credentials.filter((credential) => favoriteCredentialIds.has(credential.id))
        : credentials,
    [credentials, favoriteCredentialIds, favoritesOnly],
  );
  const tags = useMemo(
    () => [...new Set(availableCredentials.flatMap((credential) => credential.tags))].sort(),
    [availableCredentials],
  );
  const filteredCredentials = useMemo(() => {
    const query = normalizeSearchText(search);
    const selectedTag = normalizeSearchText(tag);

    return availableCredentials.filter((credential) => {
      const searchValues = [credential.name, credential.username, credential.website ?? "", ...credential.tags];
      const matchesSearch =
        !query || searchValues.some((value) => normalizeSearchText(value).includes(query));
      return (
        matchesSearch &&
        (!selectedTag || credential.tags.some((value) => normalizeSearchText(value) === selectedTag))
      );
    });
  }, [availableCredentials, search, tag]);

  const addModal = useModalClose(() => {
    addDialogGeneration.current += 1;
    setIsAdding(false);
  });

  async function createCredential(input: CredentialInput) {
    const generation = addDialogGeneration.current;
    await vaultClient.createCredential(input);
    if (addDialogGeneration.current !== generation) {
      return;
    }
    await loadRecords();
    if (addDialogGeneration.current !== generation) {
      return;
    }
    setIsAddingPending(false);
    addModal.close();
  }

  function openAddDialog() {
    addDialogGeneration.current += 1;
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
            {availableCredentials.length}{" "}
            {availableCredentials.length === 1 ? "credential" : "credentials"}
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
      {!isLoading && !loadError && credentials.length === 0 ? (
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
      credentials.length > 0 &&
      availableCredentials.length === 0 ? (
        <section className="vault-empty-state">
          <h2>No favorite credentials</h2>
          <p>Use the Star on a credential to add it here.</p>
        </section>
      ) : null}
      {!isLoading &&
      !loadError &&
      availableCredentials.length > 0 &&
      filteredCredentials.length === 0 ? (
        <p className="vault-status">No matching credentials</p>
      ) : null}
      {!isLoading && !loadError && filteredCredentials.length > 0 ? (
        viewMode === "card" ? (
          <CredentialCardStack
            credentials={filteredCredentials}
            favoriteCredentialIds={favoriteCredentialIds}
            onOpenCredential={setSelectedCredentialId}
            onToggleFavorite={onToggleFavorite}
          />
        ) : (
          <CredentialList
            credentials={filteredCredentials}
            favoriteCredentialIds={favoriteCredentialIds}
            onOpenCredential={setSelectedCredentialId}
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
          <CredentialForm
            onSubmit={createCredential}
            onCancel={closeAddDialog}
            onPendingChange={setIsAddingPending}
            initialFocusRef={addNameRef}
          />
        </VaultModal>
      ) : null}
      {selectedCredentialId ? (
        <CredentialDetailsModal
          key={selectedCredentialId}
          credentialId={selectedCredentialId}
          isFavorite={favoriteCredentialIds.has(selectedCredentialId)}
          onClose={() => setSelectedCredentialId(null)}
          onChanged={loadRecords}
          onToggleFavorite={() => onToggleFavorite(selectedCredentialId)}
          fallbackFocusRef={addButtonRef}
        />
      ) : null}
    </main>
  );
}
