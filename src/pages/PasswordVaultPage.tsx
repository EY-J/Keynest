import { useEffect, useMemo, useRef, useState } from "react";
import VaultModal from "../features/vault/components/VaultModal";
import VaultRecordDialog from "../features/vault/components/VaultRecordDialog";
import VaultRecordForm from "../features/vault/components/VaultRecordForm";
import { vaultClient } from "../features/vault/vaultClient";
import type { VaultRecordInput, VaultRecordSummary } from "../features/vault/types";

const LOAD_ERROR_MESSAGE = "KeyNest could not load your vault.";

function normalized(value: string) {
  return value.trim().toLocaleLowerCase();
}

export default function PasswordVaultPage() {
  const [records, setRecords] = useState<VaultRecordSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState("");
  const [search, setSearch] = useState("");
  const [category, setCategory] = useState("");
  const [tag, setTag] = useState("");
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

  const categories = useMemo(
    () => [...new Set(records.map((record) => record.category))].sort(),
    [records],
  );
  const tags = useMemo(
    () => [...new Set(records.flatMap((record) => record.tags))].sort(),
    [records],
  );
  const filteredRecords = useMemo(() => {
    const query = normalized(search);
    const selectedCategory = normalized(category);
    const selectedTag = normalized(tag);

    return records.filter((record) => {
      const searchValues = [record.name, record.category, ...record.tags];
      const matchesSearch =
        !query || searchValues.some((value) => normalized(value).includes(query));
      return (
        matchesSearch &&
        (!selectedCategory || normalized(record.category) === selectedCategory) &&
        (!selectedTag || record.tags.some((value) => normalized(value) === selectedTag))
      );
    });
  }, [category, records, search, tag]);

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
    setIsAdding(false);
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
    addGeneration.current += 1;
    setIsAdding(false);
  }

  return (
    <main className="password-vault-page">
      <header className="vault-page-heading">
        <div>
          <p className="eyebrow">PRIVATE CREDENTIALS</p>
          <h1>Vault</h1>
          <p>
            {records.length} {records.length === 1 ? "credential" : "credentials"}
          </p>
        </div>
        <button
          ref={addButtonRef}
          data-vault-modal-fallback
          className="primary-button"
          type="button"
          onClick={openAddDialog}
        >
          Add Credential
        </button>
      </header>

      <section className="vault-controls" aria-label="Filter credentials">
        <label className="vault-search-field">
          <span>Search credentials</span>
          <input
            type="search"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
        </label>
        <label>
          <span>Category</span>
          <select
            value={category}
            onChange={(event) => setCategory(event.target.value)}
          >
            <option value="">All categories</option>
            {categories.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>Tag</span>
          <select value={tag} onChange={(event) => setTag(event.target.value)}>
            <option value="">All tags</option>
            {tags.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </label>
      </section>

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
      records.length > 0 &&
      filteredRecords.length === 0 ? (
        <p className="vault-status">No matching credentials</p>
      ) : null}
      {!isLoading && !loadError && filteredRecords.length > 0 ? (
        <section className="vault-record-grid" aria-label="Credentials">
          {filteredRecords.map((record) => (
            <button
              className="vault-record-card"
              key={record.id}
              type="button"
              onClick={() => setSelectedRecordId(record.id)}
            >
              <span className="vault-record-category">{record.category}</span>
              <strong>{record.name}</strong>
              <span>{record.username}</span>
              {record.website ? <span>{record.website}</span> : null}
              {record.tags.length ? (
                <span>{record.tags.map((item) => `#${item}`).join(" ")}</span>
              ) : null}
            </button>
          ))}
        </section>
      ) : null}

      {isAdding ? (
        <VaultModal
          titleId="add-credential-title"
          onRequestClose={closeAddDialog}
          isDismissDisabled={isAddingPending}
          initialFocusRef={addNameRef}
          fallbackFocusRef={addButtonRef}
        >
          <div className="vault-dialog-title">
            <div>
              <p className="eyebrow">PASSWORD VAULT</p>
              <h2 id="add-credential-title">Add credential</h2>
            </div>
            <button
              className="vault-close-button"
              type="button"
              onClick={closeAddDialog}
              disabled={isAddingPending}
              aria-label="Close credential"
            >
              ×
            </button>
          </div>
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
          recordId={selectedRecordId}
          onClose={() => setSelectedRecordId(null)}
          onChanged={loadRecords}
          fallbackFocusRef={addButtonRef}
        />
      ) : null}
    </main>
  );
}
