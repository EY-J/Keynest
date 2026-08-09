import { type FormEvent, useEffect, useRef, useState } from "react";
import { authClient } from "../authClient";
import { AuthClientError } from "../types";
import AuthLayout from "./AuthLayout";
import PasswordField from "./PasswordField";
import ResetDialog from "./ResetDialog";

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
  const passwordRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (cooldownMs <= 0) {
      return;
    }
    const timer = window.setTimeout(() => setCooldownMs(0), cooldownMs);
    return () => window.clearTimeout(timer);
  }, [cooldownMs]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!password || cooldownMs > 0) {
      return;
    }

    setError("");
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
        if (requestError.retryAfterMs) {
          setCooldownMs(requestError.retryAfterMs);
        }
      } else {
        setError("KeyNest could not verify the master password.");
      }
      passwordRef.current?.focus();
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <AuthLayout
      eyebrow="ENCRYPTED LOCAL VAULT"
      title="Welcome back"
      description="Enter your master password to unlock KeyNest on this device."
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
          className="primary-button auth-submit"
          disabled={isSubmitting || cooldownMs > 0 || !password}
        >
          {isSubmitting
            ? "Unlocking…"
            : cooldownMs > 0
              ? "Please wait…"
              : "Unlock KeyNest"}
        </button>

        <button
          className="auth-reset-link"
          type="button"
          onClick={() => setIsResetOpen(true)}
        >
          Forgot password? Reset KeyNest
        </button>
      </form>

      <ResetDialog
        isOpen={isResetOpen}
        onClose={() => setIsResetOpen(false)}
        onReset={onReset}
      />
    </AuthLayout>
  );
}
