import { type FormEvent, useRef, useState } from "react";
import { validateMasterPassword } from "../../../shared/security/masterPasswordPolicy";
import PasswordStrengthMeter from "../../../components/ui/PasswordStrengthMeter";
import { authClient } from "../authClient";
import { AuthClientError } from "../types";
import AuthLayout from "./AuthLayout";
import LockScreenBackground from "./LockScreenBackground";
import PasswordField from "./PasswordField";
import RecoveryKeyScreen from "./RecoveryKeyScreen";

type SetupScreenProps = {
  onCreated: () => void;
};

export default function SetupScreen({ onCreated }: SetupScreenProps) {
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [recoveryKey, setRecoveryKey] = useState("");
  const submittingRef = useRef(false);
  const policyError = validateMasterPassword(password, confirmation);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (submittingRef.current) return;
    setError("");

    if (policyError) {
      setError(policyError);
      return;
    }

    submittingRef.current = true;
    setIsSubmitting(true);
    try {
      const result = await authClient.createMasterPassword(password);
      setPassword("");
      setConfirmation("");
      if (result.status !== "unlocked" || !result.recoveryKey) {
        setError("KeyNest did not unlock after creating the master password.");
        return;
      }
      setRecoveryKey(result.recoveryKey);
    } catch (requestError) {
      setPassword("");
      setConfirmation("");
      setError(
        requestError instanceof AuthClientError
          ? requestError.message
          : "KeyNest could not create the master password.",
      );
    } finally {
      submittingRef.current = false;
      setIsSubmitting(false);
    }
  }

  if (recoveryKey) {
    return (
      <RecoveryKeyScreen
        recoveryKey={recoveryKey}
        onSaved={async () => {
          const status = await authClient.completeRecoveryKeyDisplay();
          if (status !== "unlocked") {
            throw new AuthClientError(
              "unexpected-status",
              "KeyNest could not finish recovery-key setup.",
            );
          }
          setRecoveryKey("");
          onCreated();
        }}
      />
    );
  }

  return (
    <AuthLayout
      eyebrow="FIRST-TIME SETUP"
      title="Create your master password"
      description="This password unlocks your encrypted KeyNest data on this device."
      background={<LockScreenBackground paused={false} />}
    >
      <form className="auth-form auth-setup-form" onSubmit={(event) => void submit(event)}>
        <div className="master-password-field-feedback">
          <PasswordField
            label="Master password"
            value={password}
            onChange={setPassword}
            autoComplete="new-password"
            autoFocus
            disabled={isSubmitting}
          />
          <PasswordStrengthMeter password={password} />
        </div>
        <PasswordField
          label="Confirm master password"
          value={confirmation}
          onChange={setConfirmation}
          autoComplete="new-password"
          disabled={isSubmitting}
        />

        <p className="auth-requirement">Use at least 12 characters.</p>
        <div className="auth-warning">
          <strong>Offline Recovery Key</strong>
          <span>
            KeyNest will create a one-time Recovery Key to save separately in
            case you forget this password.
          </span>
        </div>

        {error ? (
          <p className="auth-error" role="alert">
            {error}
          </p>
        ) : null}

        <button className="primary-button auth-submit" disabled={isSubmitting || !!policyError}>
          {isSubmitting ? "Creating encrypted vault…" : "Create Master Password"}
        </button>
      </form>
    </AuthLayout>
  );
}
