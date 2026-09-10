import { type RefObject, useEffect, useRef, useState } from "react";
import { Check, Copy, ExternalLink, Eye, EyeOff, Link2, LockKeyhole, Pencil, Star, Trash2, UserRound } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { vaultClient } from "../vaultClient";
import type { VaultRecord, VaultRecordInput, VaultRecordSummary } from "../types";
import VaultModal from "./VaultModal";
import { ModalCloseButton, useModalClose } from "../../../shared/components/Modal/Modal";
import ServiceIcon from "../../../shared/components/ServiceIcon";
import VaultRecordForm from "./VaultRecordForm";

type VaultRecordDialogProps = {
  recordId: string;
  isFavorite: boolean;
  onClose: () => void;
  onChanged: () => Promise<void>;
  onToggleFavorite: () => void;
  fallbackFocusRef?: RefObject<HTMLElement | null>;
};

const detailDateFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
  year: "numeric",
});

function formatDetailDate(timestamp: number) {
  return Number.isFinite(timestamp)
    ? detailDateFormatter.format(new Date(timestamp))
    : "Not available";
}

function safeWebUrl(value: string) {
  try {
    const url = new URL(value);
    return url.protocol === "https:" || url.protocol === "http:" ? url : null;
  } catch {
    return null;
  }
}

export default function VaultRecordDialog({
  recordId,
  isFavorite,
  onClose,
  onChanged,
  onToggleFavorite,
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
  const [isUsernameCopied, setIsUsernameCopied] = useState(false);
  const [isPasswordCopied, setIsPasswordCopied] = useState(false);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const editNameRef = useRef<HTMLInputElement>(null);
  const deleteConfirmationRef = useRef<HTMLInputElement>(null);
  const generationRef = useRef(0);
  const revealRequestRef = useRef(0);
  const usernameCopyFeedbackTimerRef = useRef<number | null>(null);
  const copyFeedbackTimerRef = useRef<number | null>(null);
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

  useEffect(
    () => () => {
      if (usernameCopyFeedbackTimerRef.current !== null) {
        window.clearTimeout(usernameCopyFeedbackTimerRef.current);
      }
      if (copyFeedbackTimerRef.current !== null) {
        window.clearTimeout(copyFeedbackTimerRef.current);
      }
    },
    [],
  );

  useEffect(() => {
    const generation = ++generationRef.current;
    setRecord(null);
    setError("");
    setStatus("");
    setIsUsernameCopied(false);
    setIsPasswordCopied(false);
    if (usernameCopyFeedbackTimerRef.current !== null) {
      window.clearTimeout(usernameCopyFeedbackTimerRef.current);
      usernameCopyFeedbackTimerRef.current = null;
    }
    if (copyFeedbackTimerRef.current !== null) {
      window.clearTimeout(copyFeedbackTimerRef.current);
      copyFeedbackTimerRef.current = null;
    }
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
    if (copyFeedbackTimerRef.current !== null) {
      window.clearTimeout(copyFeedbackTimerRef.current);
      copyFeedbackTimerRef.current = null;
    }
    setIsPasswordCopied(false);
    setError("");
    setStatus("");
    setIsPending(true);
    try {
      await vaultClient.copyVaultPassword(recordId);
      if (generationRef.current === generation) {
        setStatus("Password copied securely.");
        setIsPasswordCopied(true);
        copyFeedbackTimerRef.current = window.setTimeout(() => {
          copyFeedbackTimerRef.current = null;
          setIsPasswordCopied(false);
        }, 1_200);
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

  async function copyUsername() {
    const generation = generationRef.current;
    if (usernameCopyFeedbackTimerRef.current !== null) {
      window.clearTimeout(usernameCopyFeedbackTimerRef.current);
      usernameCopyFeedbackTimerRef.current = null;
    }
    setIsUsernameCopied(false);
    setError("");
    setStatus("");
    setIsPending(true);
    try {
      await vaultClient.copyVaultUsername(recordId);
      if (generationRef.current === generation) {
        setStatus("Username copied securely.");
        setIsUsernameCopied(true);
        usernameCopyFeedbackTimerRef.current = window.setTimeout(() => {
          usernameCopyFeedbackTimerRef.current = null;
          setIsUsernameCopied(false);
        }, 1_200);
      }
    } catch {
      if (generationRef.current === generation) {
        setError("KeyNest could not copy this username.");
      }
    } finally {
      if (generationRef.current === generation) {
        setIsPending(false);
      }
    }
  }

  async function openWebsite() {
    if (!record?.website) return;
    const url = safeWebUrl(record.website);
    setError("");
    setStatus("");
    if (!url) {
      setError("KeyNest could not safely open this website.");
      return;
    }

    const generation = generationRef.current;
    setIsPending(true);
    try {
      await openUrl(url);
    } catch {
      if (generationRef.current === generation) {
        setError("KeyNest could not open this website.");
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
      className={isEditing ? "add-credential-dialog edit-credential-dialog" : ""}
      width={isEditing ? 640 : 620}
      closing={modal.closing}
      onExitComplete={modal.finishClose}
      onRequestClose={close}
      isDismissDisabled={isPending}
      initialFocusRef={closeButtonRef}
      fallbackFocusRef={fallbackFocusRef}
    >
      {isEditing ? (
        <header className="vault-dialog-title">
          <div>
            <p className="eyebrow">PASSWORD VAULT</p>
            <h2 id="vault-record-dialog-title">Edit credential</h2>
          </div>
          <ModalCloseButton buttonRef={closeButtonRef} label="Close credential" onClick={close} disabled={isPending || modal.closing} />
        </header>
      ) : (
        <header className="vault-dialog-title vault-credential-header">
          <div className="vault-credential-kicker-row">
            <p className="eyebrow">PASSWORD VAULT</p>
            <ModalCloseButton buttonRef={closeButtonRef} label="Close credential" onClick={close} disabled={isPending || modal.closing} />
          </div>
          {record && !isDeleting ? (
            <div className="vault-credential-identity">
              <ServiceIcon name={record.name} website={record.website} size="large" />
              <div className="vault-credential-identity-copy">
                <h2 id="vault-record-dialog-title">{record.name}</h2>
                {record.tags.length ? (
                  <div className="vault-credential-tags" aria-label="Credential tags">
                    {record.tags.map((item) => (
                      <span className="vault-credential-tag" key={item}>{item}</span>
                    ))}
                  </div>
                ) : null}
              </div>
              <div className="vault-credential-header-side">
                <div className="vault-dialog-header-actions">
                  <button
                    className={`vault-favorite-button vault-dialog-favorite${
                      isFavorite ? " active" : ""
                    }`}
                    type="button"
                    aria-label={
                      isFavorite
                        ? "Remove credential from favorites"
                        : "Add credential to favorites"
                    }
                    aria-pressed={isFavorite}
                    title={
                      isFavorite
                        ? "Remove credential from favorites"
                        : "Add credential to favorites"
                    }
                    onClick={onToggleFavorite}
                    disabled={isPending}
                  >
                    <Star
                      size={19}
                      fill={isFavorite ? "currentColor" : "none"}
                      aria-hidden="true"
                    />
                  </button>
                  <button
                    className="vault-top-action vault-edit-action"
                    type="button"
                    aria-label="Edit credential"
                    title="Edit credential"
                    onClick={() => void loadSecret("edit")}
                    disabled={isPending}
                  >
                    <Pencil size={17} aria-hidden="true" />
                  </button>
                  <button
                    className="vault-top-action vault-delete-action"
                    type="button"
                    aria-label="Delete credential"
                    title="Delete credential"
                    onClick={() => {
                      setRevealedPassword(null);
                      setIsDeleting(true);
                    }}
                    disabled={isPending}
                  >
                    <Trash2 size={17} aria-hidden="true" />
                  </button>
                </div>
                <div className="vault-credential-dates">
                  <span>Added {formatDetailDate(record.createdAtMs)}</span>
                  <span>Last updated {formatDetailDate(record.updatedAtMs)}</span>
                </div>
              </div>
            </div>
          ) : (
            <h2 id="vault-record-dialog-title">{title}</h2>
          )}
        </header>
      )}
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
        <p className="vault-success sr-only" aria-live="polite">
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
          <div className="vault-detail-row">
            <UserRound className="vault-detail-row-icon" size={21} aria-hidden="true" />
            <div className="vault-detail-row-copy">
              <span>Username / Email</span>
              <strong>{record.username}</strong>
            </div>
            <button
              className="vault-field-action"
              type="button"
              aria-label={isUsernameCopied ? "Copied" : "Copy username"}
              title={isUsernameCopied ? "Copied" : "Copy username"}
              onClick={() => void copyUsername()}
              disabled={isPending}
            >
              {isUsernameCopied ? (
                <Check size={17} aria-hidden="true" />
              ) : (
                <Copy size={17} aria-hidden="true" />
              )}
            </button>
          </div>
          {record.website ? (
            <div className="vault-detail-row">
              <Link2 className="vault-detail-row-icon" size={21} aria-hidden="true" />
              <div className="vault-detail-row-copy">
                <span>Website</span>
                <strong>{record.website}</strong>
              </div>
              <button
                className="vault-field-action"
                type="button"
                aria-label="Open website"
                title="Open website"
                onClick={() => void openWebsite()}
                disabled={isPending}
              >
                <ExternalLink size={17} aria-hidden="true" />
              </button>
            </div>
          ) : null}
          <div className="vault-detail-row vault-password-display">
            <LockKeyhole className="vault-detail-row-icon" size={21} aria-hidden="true" />
            <div className="vault-detail-row-copy vault-password-copy">
              <span>Password</span>
              <input
                aria-label="Password"
                type={isRevealed ? "text" : "password"}
                readOnly
                tabIndex={-1}
                value={revealedPassword ?? "••••••••"}
              />
            </div>
            <div className="vault-field-actions">
              <button
                className="vault-field-action"
                type="button"
                aria-label={isRevealed ? "Hide password" : "Reveal password"}
                title={isRevealed ? "Hide password" : "Reveal password"}
                onClick={() => void loadSecret("reveal")}
                disabled={isPending}
              >
                {isRevealed ? (
                  <EyeOff size={17} aria-hidden="true" />
                ) : (
                  <Eye size={17} aria-hidden="true" />
                )}
              </button>
              <button
                className="vault-field-action"
                type="button"
                aria-label={isPasswordCopied ? "Copied" : "Copy password"}
                title={isPasswordCopied ? "Copied" : "Copy password"}
                onClick={() => void copyPassword()}
                disabled={isPending}
              >
                {isPasswordCopied ? (
                  <Check size={17} aria-hidden="true" />
                ) : (
                  <Copy size={17} aria-hidden="true" />
                )}
              </button>
            </div>
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
