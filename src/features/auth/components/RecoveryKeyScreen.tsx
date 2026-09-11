import { type FormEvent, useState } from "react";
import AuthLayout from "./AuthLayout";
import { authClient } from "../authClient";

type RecoveryKeyConfirmationProps = {
  recoveryKey: string;
  previousKeyInvalid?: boolean;
  onSaved: () => Promise<void>;
};

export function RecoveryKeyConfirmation({
  recoveryKey,
  previousKeyInvalid = false,
  onSaved,
}: RecoveryKeyConfirmationProps) {
  const keyGroups = recoveryKey.split("-");
  const finalGroup = keyGroups[keyGroups.length - 1] ?? "";
  const [verification, setVerification] = useState("");
  const [copyStatus, setCopyStatus] = useState("");
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  async function copyRecoveryKey() {
    setCopyStatus("");
    try {
      await authClient.copyRecoveryKey(recoveryKey);
      setCopyStatus(
        "Recovery Key copied temporarily. Store it somewhere separate and secure.",
      );
    } catch {
      setCopyStatus("Copy failed. Select the Recovery Key and copy it manually.");
    }
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (verification.trim().toUpperCase() !== finalGroup) {
      setError("Enter the final four-character group from your Recovery Key.");
      return;
    }
    setError("");
    setIsSubmitting(true);
    try {
      await onSaved();
    } catch (requestError) {
      setError(
        requestError instanceof Error
          ? requestError.message
          : "KeyNest could not finish recovery-key setup.",
      );
      setIsSubmitting(false);
    }
  }

  return (
    <div className="recovery-key-content">
      {previousKeyInvalid ? (
        <p className="auth-warning" role="status">
          <strong>Your previous Recovery Key is no longer valid.</strong>
          <span>Save this replacement key before continuing.</span>
        </p>
      ) : null}

      <output className="recovery-key-value" aria-label="Recovery Key">
        {recoveryKey}
      </output>

      <button
        className="secondary-button recovery-copy-button"
        type="button"
        onClick={() => void copyRecoveryKey()}
      >
        Copy Recovery Key
      </button>
      {copyStatus ? <p className="recovery-copy-status" role="status">{copyStatus}</p> : null}

      <p className="recovery-key-guidance">
        Keep this key somewhere separate from this computer. KeyNest does not
        store a readable copy and cannot reproduce this same key later.
      </p>

      <form className="auth-form" onSubmit={(event) => void submit(event)}>
        <div className="auth-field">
          <label htmlFor="recovery-key-verification">
            Enter the final key group to confirm you saved it
          </label>
          <input
            id="recovery-key-verification"
            value={verification}
            maxLength={4}
            autoComplete="off"
            autoCapitalize="characters"
            disabled={isSubmitting}
            onChange={(event) => setVerification(event.target.value)}
          />
        </div>
        {error ? <p className="auth-error" role="alert">{error}</p> : null}
        <button
          className="primary-button auth-submit"
          disabled={isSubmitting || verification.trim().toUpperCase() !== finalGroup}
        >
          {isSubmitting ? "Finishing…" : "I Have Saved My Recovery Key"}
        </button>
      </form>
    </div>
  );
}

type RecoveryKeyScreenProps = RecoveryKeyConfirmationProps;

export default function RecoveryKeyScreen(props: RecoveryKeyScreenProps) {
  return (
    <AuthLayout
      eyebrow="OFFLINE RECOVERY"
      title="Save your Recovery Key"
      description="If you forget your Master Password, this Recovery Key can restore access to your KeyNest data."
    >
      <RecoveryKeyConfirmation {...props} />
    </AuthLayout>
  );
}
