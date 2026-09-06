import { type FormEvent, useEffect, useState } from "react";
import { KeyRound } from "lucide-react";
import { authClient } from "../../auth/authClient";
import PasswordField from "../../auth/components/PasswordField";
import { RecoveryKeyContent } from "../../auth/components/RecoveryKeyScreen";
import { AuthClientError } from "../../auth/types";
import SettingsRow from "./SettingsRow";
import Modal, { useModalClose } from "../../../shared/components/Modal/Modal";

export default function RecoverySettings() {
  const [configured, setConfigured] = useState<boolean | null>(null);
  const [statusError, setStatusError] = useState("");
  const [isOpen, setIsOpen] = useState(false);
  const [currentPassword, setCurrentPassword] = useState("");
  const [recoveryKey, setRecoveryKey] = useState("");
  const [replacedExistingKey, setReplacedExistingKey] = useState(false);
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    let current = true;
    void authClient.getRecoveryStatus().then(
      (status) => {
        if (current) {
          setConfigured(status.configured);
          setStatusError("");
        }
      },
      () => {
        if (current) {
          setStatusError("KeyNest could not read the recovery status.");
        }
      },
    );
    return () => {
      current = false;
    };
  }, []);

  const modal = useModalClose(() => {
    setCurrentPassword(""); setRecoveryKey(""); setReplacedExistingKey(false); setError(""); setIsOpen(false);
  });

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!currentPassword) {
      setError("Enter your current Master Password.");
      return;
    }
    setError("");
    setIsSubmitting(true);
    try {
      const result = await authClient.regenerateRecoveryKey(currentPassword);
      setCurrentPassword("");
      if (result.status !== "unlocked" || !result.recoveryKey) {
        setError("KeyNest could not confirm the new Recovery Key.");
        return;
      }
      setReplacedExistingKey(configured === true);
      setRecoveryKey(result.recoveryKey);
      setConfigured(true);
    } catch (requestError) {
      setCurrentPassword("");
      setError(
        requestError instanceof AuthClientError
          ? requestError.message
          : "KeyNest could not create a new Recovery Key.",
      );
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <>
      <SettingsRow
        icon={KeyRound}
        title="Recovery Key"
      >
        <div className="recovery-row-controls">
          <span
            className={`recovery-status ${
              configured === null
                ? "recovery-checking"
                : configured
                  ? "recovery-configured"
                  : "recovery-missing"
            }`}
          >
            <span className="recovery-status-dot" aria-hidden="true" />
            {configured === null
              ? "Checking…"
              : configured
                ? "Configured"
                : "Not configured"}
          </span>
          <button
            className="secondary-button compact-button"
            type="button"
            disabled={configured === null || Boolean(statusError)}
            onClick={() => setIsOpen(true)}
          >
            {configured ? "Manage" : "Create"}
          </button>
        </div>
        {statusError ? (
          <span className="settings-inline-error" role="alert">
            {statusError}
          </span>
        ) : null}
      </SettingsRow>

      {isOpen ? (
        <Modal className="reset-dialog recovery-dialog" titleId="recovery-key-dialog-title"
          closing={modal.closing} onClose={() => modal.close()} onExitComplete={modal.finishClose}
          pending={isSubmitting} closeOnEscape={!recoveryKey}>
            <p className="auth-eyebrow">RECOVERY</p>
            <h2 id="recovery-key-dialog-title">
              {recoveryKey
                ? "Save your new Recovery Key"
                : "Verify your Master Password"}
            </h2>
            {recoveryKey ? (
              <RecoveryKeyContent
                recoveryKey={recoveryKey}
                previousKeyInvalid={replacedExistingKey}
                onSaved={async () => {
                  const status = await authClient.completeRecoveryKeyDisplay();
                  if (status !== "unlocked") {
                    throw new AuthClientError(
                      "unexpected-status",
                      "KeyNest could not finish recovery-key creation.",
                    );
                  }
                  modal.close();
                }}
              />
            ) : (
              <form onSubmit={(event) => void submit(event)}>
                <p>
                  Creating or replacing a Recovery Key requires your current
                  Master Password.
                </p>
                <PasswordField
                  label="Current Master Password"
                  value={currentPassword}
                  onChange={setCurrentPassword}
                  autoComplete="current-password"
                  disabled={isSubmitting}
                />
                {error ? <p className="auth-error" role="alert">{error}</p> : null}
                <div className="reset-dialog-actions">
                  <button className="keynest-button--secondary"
                    type="button"
                    disabled={isSubmitting}
                    onClick={() => {
                      modal.close();
                    }}
                  >
                    Cancel
                  </button>
                  <button
                    className="primary-button"
                    disabled={isSubmitting || !currentPassword}
                  >
                    {isSubmitting ? "Creating…" : "Create New Key"}
                  </button>
                </div>
              </form>
            )}
        </Modal>
      ) : null}
    </>
  );
}
