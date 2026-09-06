import { type FormEvent, useEffect, useRef, useState } from "react";
import { authClient } from "../authClient";
import { AuthClientError } from "../types";
import AuthLayout from "./AuthLayout";
import PasswordField from "./PasswordField";
import ResetDialog from "./ResetDialog";
import RecoverPasswordDialog from "./RecoverPasswordDialog";
import RecoveryKeyScreen from "./RecoveryKeyScreen";
import LockScreenBackground from "./LockScreenBackground";

type UnlockScreenProps = {
  onUnlocked: () => void;
  onReset: (confirmation: string) => Promise<void>;
};

export default function UnlockScreen({
  onUnlocked,
  onReset,
}: UnlockScreenProps) {
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [cooldownMs, setCooldownMs] = useState(0);
  const [isResetOpen, setIsResetOpen] = useState(false);
  const [isRecoveryOpen, setIsRecoveryOpen] = useState(false);
  const [replacementRecoveryKey, setReplacementRecoveryKey] = useState("");
  const passwordRef = useRef<HTMLInputElement>(null);
  const submittingRef = useRef(false);

  useEffect(() => {
    if (cooldownMs <= 0) {
      return;
    }
    const timer = window.setTimeout(() => setCooldownMs(0), cooldownMs);
    return () => window.clearTimeout(timer);
  }, [cooldownMs]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!password || cooldownMs > 0 || submittingRef.current) {
      return;
    }

    setError("");
    submittingRef.current = true;
    setIsSubmitting(true);
    try {
      const status = await authClient.unlock(password);
      setPassword("");
      if (status !== "unlocked") {
        setError("KeyNest did not confirm that the vault was unlocked.");
        passwordRef.current?.focus();
        return;
      }
      onUnlocked();
    } catch (requestError) {
      setPassword("");
      if (requestError instanceof AuthClientError) {
        setError(requestError.message);
        if (requestError.retryAfterMs && Number.isFinite(requestError.retryAfterMs)) {
          setCooldownMs(Math.min(30_000, Math.max(0, Math.ceil(requestError.retryAfterMs))));
        }
      } else {
        setError("KeyNest could not verify the master password.");
      }
      passwordRef.current?.focus();
    } finally {
      submittingRef.current = false;
      setIsSubmitting(false);
    }
  }

  if (replacementRecoveryKey) {
    return (
      <RecoveryKeyScreen
        recoveryKey={replacementRecoveryKey}
        previousKeyInvalid
        onSaved={async () => {
          const status = await authClient.completeRecoveryKeyDisplay();
          if (status !== "unlocked") {
            throw new AuthClientError(
              "unexpected-status",
              "KeyNest could not finish Master Password recovery.",
            );
          }
          setReplacementRecoveryKey("");
          onUnlocked();
        }}
      />
    );
  }

  return (
    <AuthLayout
      eyebrow="ENCRYPTED LOCAL VAULT"
      title="Welcome back"
      description="Enter your master password to unlock KeyNest on this device."
      background={<LockScreenBackground paused={isRecoveryOpen || isResetOpen} />}
    >
      <form className="auth-form" onSubmit={(event) => void submit(event)}>
        <PasswordField
          label="Master password"
          value={password}
          onChange={setPassword}
          autoComplete="current-password"
          autoFocus
          disabled={isSubmitting || cooldownMs > 0}
          inputRef={passwordRef}
        />

        {error ? (
          <p className="auth-error" role="alert">
            {error}
          </p>
        ) : null}

        <button
          className="primary-button auth-submit auth-unlock-button"
          disabled={isSubmitting || cooldownMs > 0 || !password}
        >
          {isSubmitting
            ? "Unlocking…"
            : cooldownMs > 0
              ? "Please wait…"
              : "Unlock KeyNest"}
        </button>

        <button
          className="auth-reset-link keynest-button--text"
          type="button"
          onClick={() => setIsRecoveryOpen(true)}
        >
          Forgot Master Password?
        </button>
      </form>

      <RecoverPasswordDialog
        isOpen={isRecoveryOpen}
        onClose={() => setIsRecoveryOpen(false)}
        onRecovered={(result) => {
          setIsRecoveryOpen(false);
          setReplacementRecoveryKey(result.recoveryKey);
        }}
        onChooseReset={() => {
          setIsRecoveryOpen(false);
          setIsResetOpen(true);
        }}
      />

      <ResetDialog
        isOpen={isResetOpen}
        onClose={() => setIsResetOpen(false)}
        onReset={onReset}
      />
    </AuthLayout>
  );
}
