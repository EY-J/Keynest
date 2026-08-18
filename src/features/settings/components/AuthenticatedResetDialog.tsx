import { type FormEvent, useEffect, useRef, useState } from "react";
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
  const dialogRef = useRef<HTMLElement>(null);

  useEffect(() => {
    if (!isOpen) {
      setCurrentPassword("");
      setConfirmation("");
      setError("");
      return;
    }
    passwordRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && !isSubmitting) {
        onClose();
        return;
      }
      if (event.key !== "Tab") {
        return;
      }
      const focusable = Array.from(
        dialogRef.current?.querySelectorAll<HTMLElement>(
          'button:not(:disabled), input:not(:disabled)',
        ) ?? [],
      );
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (!first || !last) {
        return;
      }
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, isSubmitting, onClose]);

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
      onClose();
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
    <div className="reset-dialog-backdrop">
      <section
        ref={dialogRef}
        className="reset-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="authenticated-reset-dialog-title"
      >
        <p className="auth-eyebrow danger-text">DESTRUCTIVE RESET</p>
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
            autoFocus
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
            <button type="button" disabled={isSubmitting} onClick={onClose}>
              Cancel
            </button>
            <button className="danger-button" disabled={!canReset || isSubmitting}>
              {isSubmitting ? "Resettingâ€¦" : "Reset KeyNest"}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
