import { type RefObject, useEffect, useRef, useState } from "react";
import { Check, Copy, ExternalLink, Eye, EyeOff, Link2, LockKeyhole, Pencil, Star, Trash2, UserRound } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { vaultClient } from "../vaultClient";
import type { Credential, CredentialInput, CredentialSummary } from "../types";
import VaultModal from "./VaultModal";
import { ModalCloseButton, useModalClose } from "../../../components/ui/Modal/Modal";
import ServiceLogo from "../../../components/ui/ServiceLogo";
import CredentialForm from "./CredentialForm";

type CredentialDetailsModalProps = {
  credentialId: string;
  isFavorite: boolean;
  onClose: () => void;
  onChanged: () => Promise<void>;
  onToggleFavorite: () => void;
  onRequestDelete: (credential: CredentialSummary) => void;
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

export default function CredentialDetailsModal({
  credentialId,
  isFavorite,
  onClose,
  onChanged,
  onToggleFavorite,
  onRequestDelete,
  fallbackFocusRef,
}: CredentialDetailsModalProps) {
  const [credentialSummary, setCredentialSummary] =
    useState<CredentialSummary | null>(null);
  const [credentialForEditing, setCredentialForEditing] =
    useState<Credential | null>(null);
  const [revealedPassword, setRevealedPassword] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [isLoading, setIsLoading] = useState(true);
  const isRevealed = revealedPassword !== null;
  const [isEditing, setIsEditing] = useState(false);
  const [isPending, setIsPending] = useState(false);
  const [status, setStatus] = useState("");
  const [isUsernameCopied, setIsUsernameCopied] = useState(false);
  const [isPasswordCopied, setIsPasswordCopied] = useState(false);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const editNameRef = useRef<HTMLInputElement>(null);
  const requestGenerationRef = useRef(0);
  const revealRequestRef = useRef(0);
  const usernameCopyFeedbackTimerRef = useRef<number | null>(null);
  const passwordCopyFeedbackTimerRef = useRef<number | null>(null);
  const modalClose = useModalClose(finishClosingModal);

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
  }, [credentialId]);

  useEffect(() => {
    if (revealedPassword === null) return;
    const timer = window.setTimeout(() => setRevealedPassword(null), 12_000);
    return () => window.clearTimeout(timer);
  }, [credentialId, revealedPassword]);

  useEffect(
    () => () => {
      if (usernameCopyFeedbackTimerRef.current !== null) {
        window.clearTimeout(usernameCopyFeedbackTimerRef.current);
      }
      if (passwordCopyFeedbackTimerRef.current !== null) {
        window.clearTimeout(passwordCopyFeedbackTimerRef.current);
      }
    },
    [],
  );

  useEffect(() => {
    const generation = ++requestGenerationRef.current;
    setCredentialSummary(null);
    setError("");
    setStatus("");
    setIsUsernameCopied(false);
    setIsPasswordCopied(false);
    if (usernameCopyFeedbackTimerRef.current !== null) {
      window.clearTimeout(usernameCopyFeedbackTimerRef.current);
      usernameCopyFeedbackTimerRef.current = null;
    }
    if (passwordCopyFeedbackTimerRef.current !== null) {
      window.clearTimeout(passwordCopyFeedbackTimerRef.current);
      passwordCopyFeedbackTimerRef.current = null;
    }
    setIsLoading(true);
    setRevealedPassword(null);
    setCredentialForEditing(null);
    setIsEditing(false);
    setIsPending(false);
    void vaultClient
      .getCredentialSummary(credentialId)
      .then(
        (loaded) => {
          if (requestGenerationRef.current === generation) {
            setCredentialSummary(loaded);
          }
        },
        () => {
          if (requestGenerationRef.current === generation) {
            setError("KeyNest could not load this credential.");
          }
        },
      )
      .finally(() => {
        if (requestGenerationRef.current === generation) {
          setIsLoading(false);
        }
      });
    return () => {
      // Lock/navigation unmounts this dialog; late IPC responses must stay discarded.
      requestGenerationRef.current += 1;
    };
  }, [credentialId]);

  useEffect(() => {
    queueMicrotask(() => {
      if (isEditing) {
        editNameRef.current?.focus();
      }
    });
  }, [isEditing]);

  function closeModal() {
    if (!isPending) modalClose.close();
  }

  function finishClosingModal() {
    requestGenerationRef.current += 1;
    setCredentialSummary(null);
    setRevealedPassword(null);
    setCredentialForEditing(null);
    onClose();
  }

  async function loadCredentialSecret(purpose: "reveal" | "edit") {
    if (isPending) return;
    if (purpose === "reveal" && isRevealed) {
      setRevealedPassword(null);
      return;
    }
    const generation = requestGenerationRef.current;
    const revealRequest = ++revealRequestRef.current;
    setRevealedPassword(null);
    setError("");
    setIsPending(true);
    try {
      const loaded = await vaultClient.getCredential(credentialId);
      if (requestGenerationRef.current !== generation || loaded.id !== credentialId) return;
      if (purpose === "edit") {
        setCredentialForEditing(loaded);
        setIsEditing(true);
      } else if (revealRequestRef.current === revealRequest && !document.hidden) {
        setRevealedPassword(loaded.password);
      }
    } catch {
      if (requestGenerationRef.current === generation) setError("KeyNest could not load this credential.");
    } finally {
      if (requestGenerationRef.current === generation) setIsPending(false);
    }
  }

  async function copyPassword() {
    const generation = requestGenerationRef.current;
    if (passwordCopyFeedbackTimerRef.current !== null) {
      window.clearTimeout(passwordCopyFeedbackTimerRef.current);
      passwordCopyFeedbackTimerRef.current = null;
    }
    setIsPasswordCopied(false);
    setError("");
    setStatus("");
    setIsPending(true);
    try {
      await vaultClient.copyCredentialPassword(credentialId);
      if (requestGenerationRef.current === generation) {
        setStatus("Password copied securely.");
        setIsPasswordCopied(true);
        passwordCopyFeedbackTimerRef.current = window.setTimeout(() => {
          passwordCopyFeedbackTimerRef.current = null;
          setIsPasswordCopied(false);
        }, 1_200);
      }
    } catch {
      if (requestGenerationRef.current === generation) {
        setError("KeyNest could not copy this password.");
      }
    } finally {
      if (requestGenerationRef.current === generation) {
        setIsPending(false);
      }
    }
  }

  async function copyUsername() {
    const generation = requestGenerationRef.current;
    if (usernameCopyFeedbackTimerRef.current !== null) {
      window.clearTimeout(usernameCopyFeedbackTimerRef.current);
      usernameCopyFeedbackTimerRef.current = null;
    }
    setIsUsernameCopied(false);
    setError("");
    setStatus("");
    setIsPending(true);
    try {
      await vaultClient.copyCredentialUsername(credentialId);
      if (requestGenerationRef.current === generation) {
        setStatus("Username copied securely.");
        setIsUsernameCopied(true);
        usernameCopyFeedbackTimerRef.current = window.setTimeout(() => {
          usernameCopyFeedbackTimerRef.current = null;
          setIsUsernameCopied(false);
        }, 1_200);
      }
    } catch {
      if (requestGenerationRef.current === generation) {
        setError("KeyNest could not copy this username.");
      }
    } finally {
      if (requestGenerationRef.current === generation) {
        setIsPending(false);
      }
    }
  }

  async function openWebsite() {
    if (!credentialSummary?.website) return;
    const url = safeWebUrl(credentialSummary.website);
    setError("");
    setStatus("");
    if (!url) {
      setError("KeyNest could not safely open this website.");
      return;
    }

    const generation = requestGenerationRef.current;
    setIsPending(true);
    try {
      await openUrl(url);
    } catch {
      if (requestGenerationRef.current === generation) {
        setError("KeyNest could not open this website.");
      }
    } finally {
      if (requestGenerationRef.current === generation) {
        setIsPending(false);
      }
    }
  }

  async function updateCredential(input: CredentialInput) {
    const generation = requestGenerationRef.current;
    setError("");
    await vaultClient.updateCredential(credentialId, input);
    if (requestGenerationRef.current !== generation) {
      return;
    }
    await onChanged();
    if (requestGenerationRef.current !== generation) {
      return;
    }
    modalClose.close();
  }

  function requestDeleteCredential() {
    if (!credentialSummary || isPending) return;
    const credential = credentialSummary;
    setRevealedPassword(null);
    modalClose.close(() => onRequestDelete(credential));
  }

  const title = credentialSummary?.name ?? "Credential";
  return (
    <VaultModal
      titleId="vault-record-dialog-title"
      className={isEditing ? "add-credential-dialog edit-credential-dialog" : ""}
      width={isEditing ? 640 : 620}
      closing={modalClose.closing}
      onExitComplete={modalClose.finishClose}
      onRequestClose={closeModal}
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
          <ModalCloseButton buttonRef={closeButtonRef} label="Close credential" onClick={closeModal} disabled={isPending || modalClose.closing} />
        </header>
      ) : (
        <header className="vault-dialog-title vault-credential-header">
          <div className="vault-credential-kicker-row">
            <p className="eyebrow">PASSWORD VAULT</p>
            <ModalCloseButton buttonRef={closeButtonRef} label="Close credential" onClick={closeModal} disabled={isPending || modalClose.closing} />
          </div>
          {credentialSummary ? (
            <div className="vault-credential-identity">
              <ServiceLogo name={credentialSummary.name} website={credentialSummary.website} size="large" />
              <div className="vault-credential-identity-copy">
                <h2 id="vault-record-dialog-title">{credentialSummary.name}</h2>
                {credentialSummary.tags.length ? (
                  <div className="vault-credential-tags" aria-label="Credential tags">
                    {credentialSummary.tags.map((tag) => (
                      <span className="vault-credential-tag" key={tag}>{tag}</span>
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
                    onClick={() => void loadCredentialSecret("edit")}
                    disabled={isPending}
                  >
                    <Pencil size={17} aria-hidden="true" />
                  </button>
                  <button
                    className="vault-top-action vault-delete-action"
                    type="button"
                    aria-label="Delete credential"
                    title="Delete credential"
                    onClick={requestDeleteCredential}
                    disabled={isPending}
                  >
                    <Trash2 size={17} aria-hidden="true" />
                  </button>
                </div>
                <div className="vault-credential-dates">
                  <span>Added {formatDetailDate(credentialSummary.createdAtMs)}</span>
                  <span>Last updated {formatDetailDate(credentialSummary.updatedAtMs)}</span>
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
      {error && !isEditing ? (
        <p className="vault-form-error" aria-live="assertive">
          {error}
        </p>
      ) : null}
      {status ? (
        <p className="vault-success sr-only" aria-live="polite">
          {status}
        </p>
      ) : null}
      {credentialForEditing && isEditing ? (
        <CredentialForm
          initialRecord={credentialForEditing}
          onSubmit={updateCredential}
          onCancel={() => { setIsEditing(false); setCredentialForEditing(null); }}
          onPendingChange={setIsPending}
          initialFocusRef={editNameRef}
        />
      ) : null}
      {credentialSummary && !isEditing ? (
        <div className="vault-record-detail">
          <div className="vault-detail-row">
            <UserRound className="vault-detail-row-icon" size={21} aria-hidden="true" />
            <div className="vault-detail-row-copy">
              <span>Username / Email</span>
              <strong>{credentialSummary.username}</strong>
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
          {credentialSummary.website ? (
            <div className="vault-detail-row">
              <Link2 className="vault-detail-row-icon" size={21} aria-hidden="true" />
              <div className="vault-detail-row-copy">
                <span>Website</span>
                <strong>{credentialSummary.website}</strong>
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
                onClick={() => void loadCredentialSecret("reveal")}
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
    </VaultModal>
  );
}
