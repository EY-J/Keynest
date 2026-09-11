import { type FormEvent, useRef, useState } from "react";
import { LockKeyhole } from "lucide-react";
import { authClient } from "../../auth/authClient";
import PasswordField from "../../auth/components/PasswordField";
import { AuthClientError } from "../../auth/types";
import SettingsRow from "./SettingsRow";
import { validateMasterPassword } from "../../../shared/security/masterPasswordPolicy";
import PasswordStrengthMeter from "../../../components/ui/PasswordStrengthMeter";
import Modal, { ModalCloseButton, useModalClose } from "../../../components/ui/Modal/Modal";
import "./ChangeMasterPasswordForm.css";

const SUCCESS_MESSAGE =
  "Master Password changed. Your new password will be required after KeyNest locks.";

export default function ChangeMasterPasswordForm() {
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isOpen, setIsOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const currentPasswordRef = useRef<HTMLInputElement>(null);
  const submittingRef = useRef(false);
  const modal = useModalClose(() => {
    setCurrentPassword(""); setNewPassword(""); setConfirmation(""); setError(""); setIsOpen(false);
  });
  const isClosing = modal.closing;
  const policyError = validateMasterPassword(newPassword, confirmation);
  const passwordsMismatch = confirmation.length > 0 && confirmation !== newPassword;
  const canSubmit = currentPassword.length > 0 && !policyError && !isSubmitting && !isClosing;

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (submittingRef.current || isClosing) return;
    setError("");
    setSuccess("");

    if (!currentPassword) {
      setError("Enter your current Master Password.");
      return;
    }
    if (policyError) {
      setError(policyError);
      return;
    }

    submittingRef.current = true;
    setIsSubmitting(true);
    try {
      const status = await authClient.changeMasterPassword(
        currentPassword,
        newPassword,
      );
      if (status !== "unlocked") {
        setError("KeyNest could not confirm that the Master Password was changed.");
        return;
      }
      setSuccess(SUCCESS_MESSAGE);
      modal.close();
    } catch (requestError) {
      setError(
        requestError instanceof AuthClientError
          ? requestError.message
          : "KeyNest could not change the Master Password.",
      );
    } finally {
      setCurrentPassword("");
      setNewPassword("");
      setConfirmation("");
      submittingRef.current = false;
      setIsSubmitting(false);
    }
  }

  function closeForm() {
    if (!submittingRef.current) modal.close();
  }

  return (
    <>
      <SettingsRow
        icon={LockKeyhole}
        title="Master Password"
      >
        <button
          className="secondary-button compact-button"
          type="button"
          ref={triggerRef}
          aria-haspopup="dialog"
          aria-expanded={isOpen}
          aria-controls={isOpen ? "change-master-password-dialog" : undefined}
          onClick={() => {
            setSuccess("");
            setIsOpen(true);
          }}
        >
          Change
        </button>
      </SettingsRow>

      {success && !isOpen ? <p className="security-success" role="status">{success}</p> : null}

      {isOpen ? (
        <Modal id="change-master-password-dialog" className="master-password-modal"
          titleId="change-master-password-title" closing={isClosing} onClose={closeForm}
          onExitComplete={modal.finishClose} pending={isSubmitting} closeOnBackdrop
          initialFocusRef={currentPasswordRef} fallbackFocusRef={triggerRef}>
          <header className="master-password-modal__header">
            <h2 id="change-master-password-title">Change Master Password</h2>
            <ModalCloseButton label="Close Change Master Password" disabled={isSubmitting || isClosing} onClick={closeForm} />
          </header>
          <form
            className="master-password-modal__content"
            onSubmit={(event) => void submit(event)}
          >
            <PasswordField
              label="Current Master Password"
              value={currentPassword}
              onChange={setCurrentPassword}
              autoComplete="current-password"
              disabled={isSubmitting || isClosing}
              inputRef={currentPasswordRef}
            />
            <div className="master-password-field-feedback">
              <PasswordField
                label="New Master Password"
                value={newPassword}
                onChange={setNewPassword}
                autoComplete="new-password"
                disabled={isSubmitting || isClosing}
              />
              <PasswordStrengthMeter password={newPassword} />
            </div>
            <div className="master-password-field-feedback">
              <PasswordField
                label="Confirm New Master Password"
                value={confirmation}
                onChange={setConfirmation}
                autoComplete="new-password"
                disabled={isSubmitting || isClosing}
              />
              {passwordsMismatch ? (
                <p className="master-password-modal__mismatch" role="status">Passwords do not match.</p>
              ) : null}
            </div>
            <p className="master-password-modal__helper">12 characters minimum.</p>
            {error ? <p className="master-password-modal__error" role="alert">{error}</p> : null}
            <div className="master-password-modal__footer">
              <button
                className="secondary-button compact-button"
                type="button"
                disabled={isSubmitting || isClosing}
                onClick={closeForm}
              >
                Cancel
              </button>
              <button className="primary-button compact-button" disabled={!canSubmit}>
                {isSubmitting ? "Changing…" : "Change Password"}
              </button>
            </div>
          </form>
        </Modal>
      ) : null}
    </>
  );
}
