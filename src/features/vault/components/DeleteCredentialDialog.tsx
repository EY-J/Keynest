import { type RefObject, useEffect, useRef, useState } from "react";
import { useModalClose } from "../../../components/ui/Modal/Modal";
import ServiceLogo from "../../../components/ui/ServiceLogo";
import { useToast } from "../../../components/ui/Toast/ToastProvider";
import { vaultClient } from "../vaultClient";
import type { CredentialSummary } from "../types";
import VaultModal from "./VaultModal";

type DeleteCredentialDialogProps = {
  credential: CredentialSummary;
  fallbackFocusRef?: RefObject<HTMLElement | null>;
  onCancel: () => void;
  onDeleted: () => Promise<void>;
};

export default function DeleteCredentialDialog({
  credential,
  fallbackFocusRef,
  onCancel,
  onDeleted,
}: DeleteCredentialDialogProps) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const cancelRef = useRef<HTMLButtonElement>(null);
  const requestGenerationRef = useRef(0);
  const modal = useModalClose(onCancel);
  const { showToast } = useToast();

  useEffect(() => () => {
    requestGenerationRef.current += 1;
  }, []);

  function close() {
    if (!pending) modal.close();
  }

  async function confirmDelete() {
    const generation = requestGenerationRef.current;
    setError("");
    setPending(true);
    try {
      await vaultClient.deleteCredential(credential.id);
      if (requestGenerationRef.current !== generation) return;
      showToast({ type: "success", message: "Moved to Recently Deleted" });
      await onDeleted();
      if (requestGenerationRef.current !== generation) return;
      setPending(false);
      modal.close();
    } catch {
      if (requestGenerationRef.current === generation) {
        setError("KeyNest could not delete this credential.");
        setPending(false);
      }
    }
  }

  return (
    <VaultModal
      titleId="delete-credential-title"
      className="credential-delete-dialog"
      width={500}
      closing={modal.closing}
      onExitComplete={modal.finishClose}
      onRequestClose={close}
      isDismissDisabled={pending}
      initialFocusRef={cancelRef}
      fallbackFocusRef={fallbackFocusRef}
    >
      <h2 id="delete-credential-title">Delete credential?</h2>
      <p className="credential-delete-message">
        This credential will move to Recently Deleted for 30 days.
      </p>
      <div className="credential-delete-preview">
        <ServiceLogo name={credential.name} website={credential.website} size="medium" />
        <div className="credential-delete-preview-copy">
          <p className="credential-delete-preview-title" title={credential.name}>
            {credential.name}
          </p>
          <p className="credential-delete-preview-type">Credential</p>
        </div>
      </div>
      {error ? (
        <p className="vault-form-error" role="alert">
          {error}
        </p>
      ) : null}
      <div className="vault-dialog-actions">
        <button
          ref={cancelRef}
          className="secondary-button"
          type="button"
          onClick={close}
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
          Delete
        </button>
      </div>
    </VaultModal>
  );
}
