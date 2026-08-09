import { type FormEvent, useEffect, useRef, useState } from "react";

type ResetDialogProps = {
  isOpen: boolean;
  onClose: () => void;
  onReset: (confirmation: string) => Promise<void>;
};

export default function ResetDialog({
  isOpen,
  onClose,
  onReset,
}: ResetDialogProps) {
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!isOpen) {
      setConfirmation("");
      setError("");
      return;
    }
    inputRef.current?.focus();

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape" && !isSubmitting) {
        onClose();
      }
    }

    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [isOpen, isSubmitting, onClose]);

  if (!isOpen) {
    return null;
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (confirmation !== "RESET") {
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
    <div className="reset-dialog-backdrop">
      <section
        className="reset-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="reset-dialog-title"
      >
        <p className="auth-eyebrow danger-text">DESTRUCTIVE RESET</p>
        <h2 id="reset-dialog-title">Reset KeyNest?</h2>
        <p>
          Your encrypted profile and vault will be permanently deleted. The
          current master password cannot be recovered.
        </p>

        <form onSubmit={(event) => void submit(event)}>
          <label htmlFor="reset-confirmation">Type RESET to confirm</label>
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
            <button type="button" disabled={isSubmitting} onClick={onClose}>
              Cancel
            </button>
            <button
              className="danger-button"
              disabled={confirmation !== "RESET" || isSubmitting}
            >
              {isSubmitting ? "Resetting…" : "Reset KeyNest"}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
