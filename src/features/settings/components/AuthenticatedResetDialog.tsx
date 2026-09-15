import { type FormEvent, useEffect, useRef, useState } from "react";
import Modal, { useModalClose } from "../../../components/ui/Modal/Modal";
import PasswordField from "../../auth/components/PasswordField";

type AuthenticatedResetDialogProps = {
  isOpen: boolean;
  onClose: () => void;
  onReset: (currentPassword: string, confirmation: "RESET KEYNEST") => Promise<void>;
};

export default function AuthenticatedResetDialog({
  isOpen,
  onClose,
  onReset,
}: AuthenticatedResetDialogProps) {
  const [currentPassword, setCurrentPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const passwordRef = useRef<HTMLInputElement>(null);

  const modal = useModalClose(onClose);

  useEffect(() => {
    if (!isOpen) {
      setCurrentPassword("");
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
    if (!currentPassword || confirmation !== "RESET KEYNEST") {
      return;
    }

    setError("");
    setIsSubmitting(true);
    try {
      await onReset(currentPassword, "RESET KEYNEST");
      setCurrentPassword("");
      setConfirmation("");
      modal.close();
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

  const canReset = currentPassword.length > 0 && confirmation === "RESET KEYNEST";

  return (
    <Modal className="reset-dialog" width={440}
      titleId="authenticated-reset-dialog-title" closing={modal.closing} onClose={() => modal.close()}
      onExitComplete={modal.finishClose} pending={isSubmitting} initialFocusRef={passwordRef}>
        <h2 id="authenticated-reset-dialog-title">Reset KeyNest?</h2>
        <p>
          This permanently erases your encrypted profile and vault, then returns
          KeyNest to first-time setup.
        </p>
        <form onSubmit={(event) => void submit(event)}>
          <PasswordField
            label="Current master password"
            value={currentPassword}
            onChange={setCurrentPassword}
            autoComplete="current-password"
            disabled={isSubmitting}
            inputRef={passwordRef}
          />
          <label htmlFor="authenticated-reset-confirmation">
            Type RESET KEYNEST to confirm
          </label>
          <input
            id="authenticated-reset-confirmation"
            value={confirmation}
            autoComplete="off"
            disabled={isSubmitting}
            onChange={(event) => setConfirmation(event.target.value)}
          />
          {error ? <p className="auth-error" role="alert">{error}</p> : null}
          <div className="reset-dialog-actions">
            <button className="keynest-button--secondary" type="button" disabled={isSubmitting} onClick={() => modal.close()}>
              Cancel
            </button>
            <button className="danger-button" disabled={!canReset || isSubmitting}>
              {isSubmitting ? "Resetting…" : "Reset KeyNest"}
            </button>
          </div>
        </form>
    </Modal>
  );
}
