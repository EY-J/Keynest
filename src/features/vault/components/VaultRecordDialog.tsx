import { type RefObject, useEffect, useRef, useState } from "react";
import { vaultClient } from "../vaultClient";
import type { VaultRecord, VaultRecordInput, VaultRecordSummary } from "../types";
import VaultModal from "./VaultModal";
import { ModalCloseButton, useModalClose } from "../../../shared/components/Modal/Modal";
import VaultRecordForm from "./VaultRecordForm";

type VaultRecordDialogProps = {
  recordId: string;
  onClose: () => void;
  onChanged: () => Promise<void>;
  fallbackFocusRef?: RefObject<HTMLElement | null>;
};

export default function VaultRecordDialog({
  recordId,
  onClose,
  onChanged,
  fallbackFocusRef,
}: VaultRecordDialogProps) {
  const [record, setRecord] = useState<VaultRecordSummary | null>(null);
  const [editRecord, setEditRecord] = useState<VaultRecord | null>(null);
  const [revealedPassword, setRevealedPassword] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [isLoading, setIsLoading] = useState(true);
  const isRevealed = revealedPassword !== null;
  const [isEditing, setIsEditing] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [deleteConfirmation, setDeleteConfirmation] = useState("");
  const [isPending, setIsPending] = useState(false);
  const [status, setStatus] = useState("");
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const editNameRef = useRef<HTMLInputElement>(null);
  const deleteConfirmationRef = useRef<HTMLInputElement>(null);
  const generationRef = useRef(0);
  const revealRequestRef = useRef(0);
  const modal = useModalClose(finishClose);

  useEffect(() => {
    function hide() {
      revealRequestRef.current += 1;
      setRevealedPassword(null);
    }
    function hideWhenBackgrounded() {
      if (document.hidden) hide();
    }
    window.addEventListener("blur", hide);
    document.addEventListener("visibilitychange", hideWhenBackgrounded);
    return () => {
      revealRequestRef.current += 1;
      window.removeEventListener("blur", hide);
      document.removeEventListener("visibilitychange", hideWhenBackgrounded);
    };
  }, [recordId]);

  useEffect(() => {
    if (revealedPassword === null) return;
    const timer = window.setTimeout(() => setRevealedPassword(null), 12_000);
    return () => window.clearTimeout(timer);
  }, [revealedPassword, recordId]);

  useEffect(() => {
    const generation = ++generationRef.current;
    setRecord(null);
    setError("");
    setStatus("");
    setIsLoading(true);
    setRevealedPassword(null);
    setEditRecord(null);
    setIsEditing(false);
    setIsDeleting(false);
    setDeleteConfirmation("");
    setIsPending(false);
    void vaultClient
      .getVaultRecordSummary(recordId)
      .then(
        (loaded) => {
          if (generationRef.current === generation) {
            setRecord(loaded);
          }
        },
        () => {
          if (generationRef.current === generation) {
            setError("KeyNest could not load this credential.");
          }
        },
      )
      .finally(() => {
        if (generationRef.current === generation) {
          setIsLoading(false);
        }
      });
    return () => {
      // Lock/navigation unmounts this dialog; late IPC responses must stay discarded.
      generationRef.current += 1;
    };
  }, [recordId]);

  useEffect(() => {
    queueMicrotask(() => {
      if (isEditing) {
        editNameRef.current?.focus();
      } else if (isDeleting) {
        deleteConfirmationRef.current?.focus();
      }
    });
  }, [isDeleting, isEditing]);

  function close() {
    if (!isPending) modal.close();
  }

  function finishClose() {
    generationRef.current += 1;
    setRecord(null);
    setRevealedPassword(null);
    setEditRecord(null);
    onClose();
  }

  async function loadSecret(purpose: "reveal" | "edit") {
    if (isPending) return;
    if (purpose === "reveal" && isRevealed) {
      setRevealedPassword(null);
      return;
    }
    const generation = generationRef.current;
    const revealRequest = ++revealRequestRef.current;
    setRevealedPassword(null);
    setError("");
    setIsPending(true);
    try {
      const loaded = await vaultClient.getVaultRecord(recordId);
      if (generationRef.current !== generation || loaded.id !== recordId) return;
      if (purpose === "edit") {
        setEditRecord(loaded);
        setIsEditing(true);
      } else if (revealRequestRef.current === revealRequest && !document.hidden) {
        setRevealedPassword(loaded.password);
      }
    } catch {
      if (generationRef.current === generation) setError("KeyNest could not load this credential.");
    } finally {
      if (generationRef.current === generation) setIsPending(false);
    }
  }

  async function copyPassword() {
    const generation = generationRef.current;
    setError("");
    setStatus("");
    setIsPending(true);
    try {
      await vaultClient.copyVaultPassword(recordId);
      if (generationRef.current === generation) {
        setStatus("Password copied securely.");
      }
    } catch {
      if (generationRef.current === generation) {
        setError("KeyNest could not copy this password.");
      }
    } finally {
      if (generationRef.current === generation) {
        setIsPending(false);
      }
    }
  }

  async function update(input: VaultRecordInput) {
    const generation = generationRef.current;
    setError("");
    await vaultClient.updateVaultRecord(recordId, input);
    if (generationRef.current !== generation) {
      return;
    }
    await onChanged();
    if (generationRef.current !== generation) {
      return;
    }
    modal.close();
  }

  async function remove() {
    if (!record || deleteConfirmation !== record.name) {
      return;
    }
    const generation = generationRef.current;
    setError("");
    setIsPending(true);
    try {
      await vaultClient.deleteVaultRecord(recordId);
      if (generationRef.current !== generation) {
        return;
      }
      await onChanged();
      if (generationRef.current !== generation) {
        return;
      }
      modal.close();
    } catch {
      if (generationRef.current === generation) {
        setError("KeyNest could not delete this credential.");
      }
    } finally {
      if (generationRef.current === generation) {
        setIsPending(false);
      }
    }
  }

  const title = record?.name ?? "Credential";
  return (
    <VaultModal
      titleId="vault-record-dialog-title"
      closing={modal.closing}
      onExitComplete={modal.finishClose}
      onRequestClose={close}
      isDismissDisabled={isPending}
      initialFocusRef={closeButtonRef}
      fallbackFocusRef={fallbackFocusRef}
    >
      <div className="vault-dialog-title">
        <div>
          <p className="eyebrow">PASSWORD VAULT</p>
          <h2 id="vault-record-dialog-title">{title}</h2>
        </div>
        <ModalCloseButton buttonRef={closeButtonRef} label="Close credential" onClick={close} disabled={isPending || modal.closing} />
      </div>
      {isLoading ? (
        <p className="vault-status" role="status">
          Loading credential…
        </p>
      ) : null}
      {error && !isEditing && !isDeleting ? (
        <p className="vault-form-error" aria-live="assertive">
          {error}
        </p>
      ) : null}
      {status ? (
        <p className="vault-success" aria-live="polite">
          {status}
        </p>
      ) : null}
      {editRecord && isEditing ? (
        <VaultRecordForm
          initialRecord={editRecord}
          onSubmit={update}
          onCancel={() => { setIsEditing(false); setEditRecord(null); }}
          onPendingChange={setIsPending}
          initialFocusRef={editNameRef}
        />
      ) : null}
      {record && !isEditing && !isDeleting ? (
        <div className="vault-record-detail">
          <p>
            <span>Username or email</span>
            {record.username}
          </p>
          {record.website ? (
            <p>
              <span>Website</span>
              {record.website}
            </p>
          ) : null}
          {record.tags.length ? (
            <p>
              <span>Tags</span>
              {record.tags.join(", ")}
            </p>
          ) : null}
          <label className="vault-password-display">
            <span>Password</span>
            <input
              aria-label="Password"
              type={isRevealed ? "text" : "password"}
              readOnly
              value={revealedPassword ?? "••••••••"}
            />
          </label>
          <div className="vault-dialog-actions">
            <button
              className="secondary-button"
              type="button"
              onClick={() => void loadSecret("reveal")}
              disabled={isPending}
            >
              {isRevealed ? "Hide" : "Reveal"}
            </button>
            <button
              className="secondary-button"
              type="button"
              onClick={() => void copyPassword()}
              disabled={isPending}
            >
              Copy password
            </button>
          </div>
          <div className="vault-dialog-actions">
            <button
              className="secondary-button"
              type="button"
              onClick={() => void loadSecret("edit")}
              disabled={isPending}
            >
              Edit
            </button>
            <button
              className="vault-danger-button"
              type="button"
              onClick={() => { setRevealedPassword(null); setIsDeleting(true); }}
              disabled={isPending}
            >
              Delete
            </button>
          </div>
        </div>
      ) : null}
      {record && isDeleting ? (
        <section
          className="vault-delete-confirmation"
          aria-labelledby="delete-credential-title"
        >
          <h3 id="delete-credential-title">Delete {record.name} permanently?</h3>
          <p>This action cannot be undone.</p>
          <label>
            <span>Type {record.name} to confirm</span>
            <input
              ref={deleteConfirmationRef}
              value={deleteConfirmation}
              onChange={(event) => setDeleteConfirmation(event.target.value)}
              disabled={isPending}
            />
          </label>
          {error ? (
            <p className="vault-form-error" aria-live="assertive">
              {error}
            </p>
          ) : null}
          <div className="vault-dialog-actions">
            <button
              className="secondary-button"
              type="button"
              onClick={() => {
                setIsDeleting(false);
                setDeleteConfirmation("");
              }}
              disabled={isPending}
            >
              Cancel
            </button>
            <button
              className="vault-danger-button"
              type="button"
              onClick={() => void remove()}
              disabled={isPending || deleteConfirmation !== record.name}
            >
              Delete Credential
            </button>
          </div>
        </section>
      ) : null}
    </VaultModal>
  );
}
