import { type FormEvent, useState } from "react";
import { useToast } from "../../../components/ui/Toast/ToastProvider";
import AuthLayout from "./AuthLayout";
import { authClient } from "../authClient";
import "./RecoveryKeyScreen.css";

type RecoveryKeyConfirmationProps = {
  recoveryKey: string;
  previousKeyInvalid?: boolean;
  onSaved: () => Promise<void>;
  concise?: boolean;
};

const RECOVERY_GROUP_POSITION_NAMES = [
  "first",
  "second",
  "third",
  "fourth",
  "fifth",
  "sixth",
  "seventh",
] as const;

export function RecoveryKeyConfirmation({
  recoveryKey,
  previousKeyInvalid = false,
  onSaved,
  concise = false,
}: RecoveryKeyConfirmationProps) {
  const keyGroups = recoveryKey.split("-");
  const finalGroup = keyGroups[keyGroups.length - 1] ?? "";
  const confirmableGroups = keyGroups.filter((group) => group.length === 4);
  const challengeGroups = confirmableGroups.length > 0
    ? confirmableGroups
    : [finalGroup];
  const [challengeIndex] = useState(() => {
    if (challengeGroups.length < 2) return 0;
    const randomValue = new Uint32Array(1);
    globalThis.crypto.getRandomValues(randomValue);
    return randomValue[0] % challengeGroups.length;
  });
  const challengeGroup = challengeGroups[challengeIndex] ?? finalGroup;
  const challengePosition = challengeIndex + 1;
  const challengeCount = challengeGroups.length;
  const challengePositionLabel = challengeIndex === 0
    ? "first"
    : challengePosition === challengeCount
      ? "final"
      : RECOVERY_GROUP_POSITION_NAMES[challengeIndex] ?? `${challengePosition}th`;
  const [verification, setVerification] = useState("");
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const { showToast } = useToast();

  async function copyRecoveryKey() {
    try {
      await authClient.copyRecoveryKey(recoveryKey);
      showToast({ type: "success", message: "Copied to clipboard" });
    } catch {
      showToast({ type: "error", message: "Unable to copy recovery key" });
    }
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (verification.trim().toUpperCase() !== challengeGroup) {
      setError(
        `Enter the ${challengePositionLabel} 4-character group from your Recovery Key.`,
      );
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

      {!concise ? (
        <p className="recovery-key-guidance">
          Keep this key somewhere separate from this computer. KeyNest does not
          store a readable copy and cannot reproduce this same key later.
        </p>
      ) : null}

      <form
        className="auth-form recovery-key-confirmation-form"
        onSubmit={(event) => void submit(event)}
      >
        <div className="auth-field">
          <label htmlFor="recovery-key-verification">
            {concise
              ? `Enter the ${challengePositionLabel} 4-character group from your Recovery Key`
              : `Enter the ${challengePositionLabel} key group to confirm you saved it`}
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
          disabled={isSubmitting || verification.trim().toUpperCase() !== challengeGroup}
        >
          {isSubmitting
            ? "Finishing…"
            : concise
              ? "Confirm Recovery Key"
              : "I Have Saved My Recovery Key"}
        </button>
      </form>
    </div>
  );
}

type RecoveryKeyScreenProps = Omit<RecoveryKeyConfirmationProps, "concise">;

export default function RecoveryKeyScreen(props: RecoveryKeyScreenProps) {
  return (
    <AuthLayout
      eyebrow="OFFLINE RECOVERY"
      title="Save your Recovery Key"
      description="Your recovery key is generated once. Store it somewhere safe."
    >
      <div className="offline-recovery-step">
        <RecoveryKeyConfirmation {...props} concise />
      </div>
    </AuthLayout>
  );
}
