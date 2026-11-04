import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FileText, Folder as FolderIcon, KeyRound, Plus } from "lucide-react";
import { useNavigate } from "react-router-dom";
import Modal, { useModalClose } from "../../components/ui/Modal/Modal";
import { vaultClient } from "../vault/vaultClient";
import type { CredentialSummary } from "../vault/types";
import { notesClient } from "../notes/notesClient";
import type { Note } from "../notes/types";
import { foldersClient, type Folder, type FolderItem } from "./foldersClient";

export default function FoldersPage() {
  const navigate = useNavigate();
  const [folders, setFolders] = useState<Folder[]>([]);
  const [items, setItems] = useState<FolderItem[]>([]);
  const [credentials, setCredentials] = useState<CredentialSummary[]>([]);
  const [notes, setNotes] = useState<Pick<Note, "id" | "title">[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [formFolder, setFormFolder] = useState<Folder | "new" | null>(null);
  const [name, setName] = useState("");
  const [formError, setFormError] = useState("");
  const [deleteFolder, setDeleteFolder] = useState<Folder | null>(null);
  const [pending, setPending] = useState(false);
  const createRef = useRef<HTMLButtonElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const formModal = useModalClose(() => setFormFolder(null));
  const deleteModal = useModalClose(() => setDeleteFolder(null));

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const [allFolders, allItems, allCredentials, allNotes] =
        await Promise.all([
          foldersClient.list(),
          foldersClient.items(),
          vaultClient.listCredentials(),
          notesClient.listSummaries(),
        ]);
      setFolders(allFolders);
      setItems(allItems);
      setCredentials(allCredentials);
      setNotes(allNotes);
      setSelectedId((current) =>
        current && allFolders.some((folder) => folder.id === current)
          ? current
          : null,
      );
    } catch (reason) {
      setError(
        reason instanceof Error ? reason.message : "Could not load folders.",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const activeCredentialIds = useMemo(
    () => new Set(credentials.map((item) => item.id)),
    [credentials],
  );
  const activeNoteIds = useMemo(
    () => new Set(notes.map((item) => item.id)),
    [notes],
  );
  const activeItems = useMemo(
    () =>
      items.filter((item) =>
        item.kind === "credential"
          ? activeCredentialIds.has(item.itemId)
          : activeNoteIds.has(item.itemId),
      ),
    [items, activeCredentialIds, activeNoteIds],
  );
  const selectedFolder = folders.find((folder) => folder.id === selectedId);
  const selectedItems = activeItems.filter(
    (item) => item.folderId === selectedId,
  );
  const folderCredentials = credentials.filter((item) =>
    selectedItems.some(
      (member) => member.kind === "credential" && member.itemId === item.id,
    ),
  );
  const folderNotes = notes.filter((item) =>
    selectedItems.some(
      (member) => member.kind === "note" && member.itemId === item.id,
    ),
  );

  function openForm(folder: Folder | "new") {
    setFormFolder(folder);
    setName(folder === "new" ? "" : folder.name);
    setFormError("");
  }

  async function saveFolder(event: React.FormEvent) {
    event.preventDefault();
    const normalized = name.trim();
    if (!normalized || normalized.length > 100) {
      setFormError("Enter a folder name of at most 100 characters.");
      return;
    }
    setPending(true);
    setFormError("");
    try {
      if (formFolder === "new") {
        const created = await foldersClient.create(normalized);
        setSelectedId(created.id);
      } else if (formFolder)
        await foldersClient.rename(formFolder.id, normalized);
      await load();
      formModal.close();
    } catch (reason) {
      setFormError(
        reason instanceof Error ? reason.message : "Could not save folder.",
      );
    } finally {
      setPending(false);
    }
  }

  async function confirmDelete() {
    if (!deleteFolder) return;
    setPending(true);
    setFormError("");
    try {
      await foldersClient.delete(deleteFolder.id);
      await load();
      deleteModal.close();
    } catch (reason) {
      setFormError(
        reason instanceof Error ? reason.message : "Could not delete folder.",
      );
    } finally {
      setPending(false);
    }
  }

  return (
    <main className="password-vault-page folders-page">
      <header className="vault-page-heading">
        <div className="vault-heading-copy">
          <p className="eyebrow">QUICK ACCESS</p>
          <h1>Folders</h1>
          <p>
            {folders.length} {folders.length === 1 ? "folder" : "folders"}
          </p>
        </div>
        <button
          ref={createRef}
          className="primary-button"
          type="button"
          onClick={() => openForm("new")}
        >
          <Plus size={17} aria-hidden="true" /> Create folder
        </button>
      </header>
      {loading ? (
        <p className="vault-status" role="status">
          Loading folders…
        </p>
      ) : null}
      {error ? (
        <section className="vault-load-error" role="alert">
          <p>{error}</p>
          <button
            className="secondary-button"
            type="button"
            onClick={() => void load()}
          >
            Retry
          </button>
        </section>
      ) : null}
      {!loading && !error && folders.length === 0 ? (
        <section className="vault-empty-state">
          <FolderIcon size={30} aria-hidden="true" />
          <h2>No folders yet</h2>
          <p>Create a folder to organize credentials and notes.</p>
        </section>
      ) : null}
      {!loading && !error && folders.length > 0 ? (
        <div className="folders-workspace">
          <div className="folders-list" role="list" aria-label="Folders">
            {folders.map((folder) => {
              const count = activeItems.filter(
                (item) => item.folderId === folder.id,
              ).length;
              return (
                <button
                  key={folder.id}
                  role="listitem"
                  type="button"
                  className={`folders-list-item${selectedId === folder.id ? " selected" : ""}`}
                  onClick={() => setSelectedId(folder.id)}
                  aria-current={selectedId === folder.id ? "true" : undefined}
                >
                  <FolderIcon size={20} aria-hidden="true" />
                  <span>
                    <strong>{folder.name}</strong>
                    <small>
                      {count} {count === 1 ? "item" : "items"}
                    </small>
                  </span>
                </button>
              );
            })}
          </div>
          <section className="folders-detail" aria-label="Folder contents">
            {selectedFolder ? (
              <>
                <div className="folders-detail-heading">
                  <h2>{selectedFolder.name}</h2>
                  <div>
                    <button
                      className="secondary-button"
                      type="button"
                      onClick={() => openForm(selectedFolder)}
                    >
                      Rename
                    </button>
                    <button
                      className="vault-danger-button"
                      type="button"
                      onClick={() => {
                        setFormError("");
                        setDeleteFolder(selectedFolder);
                      }}
                    >
                      Delete
                    </button>
                  </div>
                </div>
                {selectedItems.length === 0 ? (
                  <div className="vault-empty-state">
                    <h2>This folder is empty</h2>
                    <p>Move a credential or note here from its details.</p>
                  </div>
                ) : (
                  <>
                    {folderCredentials.length > 0 ? (
                      <section>
                        <h3>Credentials</h3>
                        {folderCredentials.map((item) => (
                          <button
                            className="folders-content-item"
                            type="button"
                            key={item.id}
                            onClick={() =>
                              navigate("/vault", {
                                state: { openCredentialId: item.id },
                              })
                            }
                          >
                            <KeyRound size={18} aria-hidden="true" />
                            <span>{item.name}</span>
                          </button>
                        ))}
                      </section>
                    ) : null}
                    {folderNotes.length > 0 ? (
                      <section>
                        <h3>Notes</h3>
                        {folderNotes.map((item) => (
                          <button
                            className="folders-content-item"
                            type="button"
                            key={item.id}
                            onClick={() =>
                              navigate("/notes", {
                                state: { openNoteId: item.id },
                              })
                            }
                          >
                            <FileText size={18} aria-hidden="true" />
                            <span>{item.title}</span>
                          </button>
                        ))}
                      </section>
                    ) : null}
                  </>
                )}
              </>
            ) : (
              <div className="vault-empty-state">
                <h2>Select a folder</h2>
                <p>Choose a folder to see its credentials and notes.</p>
              </div>
            )}
          </section>
        </div>
      ) : null}
      {formFolder ? (
        <Modal
          titleId="folder-form-title"
          width={440}
          closing={formModal.closing}
          onClose={() => {
            if (!pending) formModal.close();
          }}
          onExitComplete={formModal.finishClose}
          pending={pending}
          initialFocusRef={inputRef}
          fallbackFocusRef={createRef}
        >
          <h2 id="folder-form-title">
            {formFolder === "new" ? "Create folder" : "Rename folder"}
          </h2>
          <form onSubmit={(event) => void saveFolder(event)}>
            <label className="folders-form-label" htmlFor="folder-name">
              Folder name
            </label>
            <input
              ref={inputRef}
              id="folder-name"
              value={name}
              maxLength={100}
              onChange={(event) => setName(event.target.value)}
              required
            />
            {formError ? (
              <p className="vault-form-error" role="alert">
                {formError}
              </p>
            ) : null}
            <div className="vault-dialog-actions">
              <button
                className="secondary-button"
                type="button"
                onClick={() => formModal.close()}
                disabled={pending}
              >
                Cancel
              </button>
              <button
                className="primary-button"
                type="submit"
                disabled={pending}
              >
                {formFolder === "new" ? "Create" : "Save"}
              </button>
            </div>
          </form>
        </Modal>
      ) : null}
      {deleteFolder ? (
        <Modal
          titleId="delete-folder-title"
          width={440}
          closing={deleteModal.closing}
          onClose={() => {
            if (!pending) deleteModal.close();
          }}
          onExitComplete={deleteModal.finishClose}
          pending={pending}
          fallbackFocusRef={createRef}
        >
          <h2 id="delete-folder-title">Delete {deleteFolder.name}?</h2>
          <p>
            {activeItems.some((item) => item.folderId === deleteFolder.id)
              ? "Items in this folder will become unfiled. Credentials and notes will remain intact."
              : "This folder will be removed."}
          </p>
          {formError ? (
            <p className="vault-form-error" role="alert">
              {formError}
            </p>
          ) : null}
          <div className="vault-dialog-actions">
            <button
              className="secondary-button"
              type="button"
              onClick={() => deleteModal.close()}
              disabled={pending}
            >
              Cancel
            </button>
            <button
              className="vault-danger-button"
              type="button"
              onClick={() => void confirmDelete()}
              disabled={pending}
            >
              Delete folder
            </button>
          </div>
        </Modal>
      ) : null}
    </main>
  );
}
