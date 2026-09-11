import { type FormEvent, useEffect, useRef, useState } from "react";
import { ChevronRight, KeyRound, TriangleAlert } from "lucide-react";
import Modal, { ModalCloseButton, useModalClose } from "../../../components/ui/Modal/Modal";
import { validateMasterPassword } from "../../../shared/security/masterPasswordPolicy";
import PasswordStrengthMeter from "../../../components/ui/PasswordStrengthMeter";
import { authClient } from "../authClient";
import { AuthClientError, type RecoveryKeyResult } from "../types";
import PasswordField from "./PasswordField";
import "./RecoverPasswordDialog.css";

type RecoverPasswordDialogProps = {
  isOpen: boolean;
  onClose: () => void;
  onRecovered: (result: RecoveryKeyResult) => void;
  onChooseReset: () => void;
};

export default function RecoverPasswordDialog({
  isOpen,
  onClose,
  onRecovered,
  onChooseReset,
}: RecoverPasswordDialogProps) {
  const [showForm, setShowForm] = useState(false);
  const [recoveryKey, setRecoveryKey] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [cooldownMs, setCooldownMs] = useState(0);
  const submittingRef = useRef(false);
  const recoveryInputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (showForm) recoveryInputRef.current?.focus({ preventScroll: true });
  }, [showForm]);
  const policyError = validateMasterPassword(newPassword, confirmation);

  useEffect(() => {
    if (cooldownMs <= 0) return;
    const timer = window.setTimeout(() => setCooldownMs(0), cooldownMs);
    return () => window.clearTimeout(timer);
  }, [cooldownMs]);

  const modal = useModalClose(onClose);

  useEffect(() => {
    if (!isOpen) {
      setShowForm(false);
      setRecoveryKey("");
      setNewPassword("");
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
    if (submittingRef.current || cooldownMs > 0) return;
    setError("");
    if (!recoveryKey.trim()) {
      setError("Enter your Recovery Key.");
      return;
    }
    if (policyError) {
      setError(policyError);
      return;
    }

    submittingRef.current = true;
    setIsSubmitting(true);
    try {
      const result = await authClient.recoverMasterPassword(
        recoveryKey,
        newPassword,
      );
      setRecoveryKey("");
      setNewPassword("");
      setConfirmation("");
      if (result.status !== "unlocked" || !result.recoveryKey) {
        setError("KeyNest could not confirm recovery.");
        return;
      }
      modal.close(() => onRecovered(result));
    } catch (requestError) {
      if (requestError instanceof AuthClientError && requestError.retryAfterMs &&
        Number.isFinite(requestError.retryAfterMs)) {
        setCooldownMs(Math.min(30_000, Math.max(0, Math.ceil(requestError.retryAfterMs))));
      }
      setRecoveryKey("");
      setNewPassword("");
      setConfirmation("");
      setError(
        requestError instanceof AuthClientError
          ? requestError.message
          : "KeyNest could not recover your Master Password.",
      );
    } finally {
      submittingRef.current = false;
      setIsSubmitting(false);
    }
  }

  return (
    <Modal className="reset-dialog recovery-dialog" width={520}
      titleId="recover-password-title" closing={modal.closing} onClose={() => modal.close()}
      onExitComplete={modal.finishClose} pending={isSubmitting}>
        <div className="recovery-modal-header">
          <p className="auth-eyebrow">OFFLINE RECOVERY</p>
          <ModalCloseButton className="offline-recovery-close" label="Close Offline Recovery" onClick={() => modal.close()} disabled={isSubmitting || modal.closing} />
        </div>
        <h2 id="recover-password-title">Forgot your Master Password?</h2>

        {!showForm ? (
          <div className="recovery-choice-list">
            <p>Use your Recovery Key to regain access to KeyNest.</p>
            <div className="recovery-options">
              <button type="button" className="recovery-option recovery-option--primary" onClick={() => setShowForm(true)} disabled={isSubmitting || modal.closing}>
                <span className="recovery-option-icon"><KeyRound size={18} aria-hidden="true" /></span>
                <span className="recovery-option-copy">
                  <strong>Recovery Key</strong>
                  <span>Use the key you saved during setup.</span>
                </span>
                <ChevronRight className="recovery-option-chevron" size={18} aria-hidden="true" />
              </button>
              <button type="button" className="recovery-option recovery-option--danger" onClick={() => modal.close(onChooseReset)} disabled={isSubmitting || modal.closing}>
                <span className="recovery-option-icon"><TriangleAlert size={18} aria-hidden="true" /></span>
                <span className="recovery-option-copy">
                  <strong>Erase &amp; Reset</strong>
                  <span>Lost both your Master Password and Recovery Key? This permanently deletes your local KeyNest data.</span>
                </span>
                <ChevronRight className="recovery-option-chevron" size={18} aria-hidden="true" />
              </button>
            </div>
          </div>
        ) : (
          <form onSubmit={(event) => void submit(event)}>
            <PasswordField
              label="Recovery Key"
              inputRef={recoveryInputRef}
              value={recoveryKey}
              onChange={setRecoveryKey}
              autoComplete="off"
              disabled={isSubmitting}
            />
            <div className="master-password-field-feedback">
              <PasswordField
                label="New Master Password"
                value={newPassword}
                onChange={setNewPassword}
                autoComplete="new-password"
                disabled={isSubmitting}
              />
              <PasswordStrengthMeter password={newPassword} />
            </div>
            <PasswordField
              label="Confirm new Master Password"
              value={confirmation}
              onChange={setConfirmation}
              autoComplete="new-password"
              disabled={isSubmitting}
            />
            <p className="auth-requirement">Use at least 12 characters.</p>
            {error ? <p className="auth-error" role="alert">{error}</p> : null}
            <div className="reset-dialog-actions">
              <button className="keynest-button--secondary" type="button" disabled={isSubmitting} onClick={() => setShowForm(false)}>
                Back
              </button>
              <button className="primary-button" disabled={isSubmitting || cooldownMs > 0 || !recoveryKey.trim() || !!policyError}>
                {isSubmitting ? "Recovering…" : cooldownMs > 0 ? "Please wait…" : "Recover KeyNest"}
              </button>
            </div>
          </form>
        )}
    </Modal>
  );
}
