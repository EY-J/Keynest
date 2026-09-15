import { type FormEvent, useEffect, useRef, useState } from "react";
import Modal, { useModalClose } from "../../../components/ui/Modal/Modal";

type UnauthenticatedResetDialogProps = {
  isOpen: boolean;
  onClose: () => void;
  onReset: (confirmation: string) => Promise<void>;
};

export default function UnauthenticatedResetDialog({
  isOpen,
  onClose,
  onReset,
}: UnauthenticatedResetDialogProps) {
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const modal = useModalClose(onClose);

  useEffect(() => {
    if (!isOpen) {
      setConfirmation("");
      setError("");
      return;
    }
  }, [isOpen]);

  if (!isOpen) {
    return null;
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (confirmation !== "RESET KEYNEST") {
      return;
    }

    setError("");
    setIsSubmitting(true);
    try {
      await onReset(confirmation);
      setConfirmation("");
    } catch (requestError) {
      setError(
        requestError instanceof Error
          ? requestError.message
          : "KeyNest could not remove its local encrypted data.",
      );
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <Modal className="reset-dialog" width={440}
      titleId="reset-dialog-title" closing={modal.closing} onClose={() => modal.close()}
      onExitComplete={modal.finishClose} pending={isSubmitting} initialFocusRef={inputRef}>
        <h2 id="reset-dialog-title">Reset KeyNest?</h2>
        <p>
          This permanently erases your encrypted profile and vault. It does
          not unlock your existing data.
        </p>

        <form onSubmit={(event) => void submit(event)}>
          <label htmlFor="reset-confirmation">
            Type RESET KEYNEST to confirm
          </label>
          <input
            ref={inputRef}
            id="reset-confirmation"
            value={confirmation}
            autoComplete="off"
            disabled={isSubmitting}
            onChange={(event) => setConfirmation(event.target.value)}
          />

          {error ? (
            <p className="auth-error" role="alert">
              {error}
            </p>
          ) : null}

          <div className="reset-dialog-actions">
            <button className="keynest-button--secondary" type="button" disabled={isSubmitting} onClick={() => modal.close()}>
              Cancel
            </button>
            <button
              className="danger-button"
              disabled={confirmation !== "RESET KEYNEST" || isSubmitting}
            >
              {isSubmitting ? "Resetting…" : "Reset KeyNest"}
            </button>
          </div>
        </form>
    </Modal>
  );
}
