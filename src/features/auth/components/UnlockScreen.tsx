import { type FormEvent, useEffect, useRef, useState } from "react";
import { authClient } from "../authClient";
import { AuthClientError } from "../types";
import AuthLayout from "./AuthLayout";
import PasswordField from "./PasswordField";
import UnauthenticatedResetDialog from "./UnauthenticatedResetDialog";
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
  const [pin, setPin] = useState("");
  const [mode, setMode] = useState<"password" | "pin">("password");
  const [pinConfigured, setPinConfigured] = useState(false);
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [cooldownMs, setCooldownMs] = useState(0);
  const [isResetOpen, setIsResetOpen] = useState(false);
  const [isRecoveryOpen, setIsRecoveryOpen] = useState(false);
  const [replacementRecoveryKey, setReplacementRecoveryKey] = useState("");
  const passwordRef = useRef<HTMLInputElement>(null);
  const pinRef = useRef<HTMLInputElement>(null);
  const submittingRef = useRef(false);

  useEffect(() => {
    let current = true;
    void authClient.getPinStatus().then(
      (status) => {
        if (!current) return;
        setPinConfigured(status.configured && status.unlockAvailable);
        if (status.configured && status.unlockAvailable) setMode("pin");
      },
      () => undefined,
    );
    return () => { current = false; };
  }, []);

  useEffect(() => {
    if (cooldownMs <= 0) {
      return;
    }
    const timer = window.setTimeout(() => setCooldownMs(0), cooldownMs);
    return () => window.clearTimeout(timer);
  }, [cooldownMs]);

  useEffect(() => {
    (mode === "pin" ? pinRef : passwordRef).current?.focus();
  }, [mode]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const credential = mode === "pin" ? pin : password;
    if (!credential || cooldownMs > 0 || submittingRef.current) {
      return;
    }

    setError("");
    submittingRef.current = true;
    setIsSubmitting(true);
    try {
      const status = mode === "pin"
        ? await authClient.unlockWithPin(pin)
        : await authClient.unlock(password);
      setPassword("");
      setPin("");
      if (status !== "unlocked") {
        setError("KeyNest did not confirm that the vault was unlocked.");
        (mode === "pin" ? pinRef : passwordRef).current?.focus();
        return;
      }
      onUnlocked();
    } catch (requestError) {
      setPassword("");
      setPin("");
      if (requestError instanceof AuthClientError) {
        setError(requestError.message);
        if (requestError.code === "pin-requires-master-password") {
          setPinConfigured(false);
          setMode("password");
        }
        if (requestError.retryAfterMs && Number.isFinite(requestError.retryAfterMs)) {
          setCooldownMs(Math.min(30_000, Math.max(0, Math.ceil(requestError.retryAfterMs))));
        }
      } else {
        setError(mode === "pin"
          ? "KeyNest could not verify the device PIN."
          : "KeyNest could not verify the Master Password.");
      }
      (requestError instanceof AuthClientError && requestError.code === "pin-requires-master-password"
        ? passwordRef
        : mode === "pin" ? pinRef : passwordRef).current?.focus();
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
      eyebrow="KeyNest"
      title="Welcome back"
      background={<LockScreenBackground paused={isRecoveryOpen || isResetOpen} />}
    >
      <form
        className={`auth-form ${mode === "pin" ? "auth-pin-form" : "auth-master-form"}`}
        onSubmit={(event) => void submit(event)}
      >
        <div
          className={`auth-mode-label auth-mode-label--${mode === "pin" ? "pin" : "password"}`}
          aria-hidden="true"
        >
          {mode === "pin" ? "Enter PIN" : "Enter Master Password"}
        </div>

        {mode === "pin" ? (
          <div className="auth-field auth-pin-field">
            <label className="sr-only" htmlFor="device-pin-unlock">Device PIN</label>
            <input
              ref={pinRef}
              id="device-pin-unlock"
              type="password"
              inputMode="numeric"
              pattern="[0-9]{6}"
              maxLength={6}
              value={pin}
              autoComplete="off"
              autoFocus
              disabled={isSubmitting || cooldownMs > 0}
              onChange={(event) => setPin(event.target.value.replace(/\D/g, "").slice(0, 6))}
            />
          </div>
        ) : (
          <PasswordField
            label="Master Password"
            value={password}
            onChange={setPassword}
            autoComplete="current-password"
            autoFocus
            disabled={isSubmitting || cooldownMs > 0}
            inputRef={passwordRef}
            visuallyHideLabel
          />
        )}

        {error ? (
          <p className="auth-error" role="alert">
            {error}
          </p>
        ) : null}

        <button
          className="primary-button auth-submit auth-unlock-button"
          disabled={isSubmitting || cooldownMs > 0 || (mode === "pin" ? pin.length !== 6 : !password)}
        >
          {isSubmitting
            ? "Unlocking…"
            : cooldownMs > 0
              ? "Please wait…"
              : "Unlock"}
        </button>

        {pinConfigured ? (
          <button
            className="auth-method-link keynest-button--text"
            type="button"
            disabled={isSubmitting}
            onClick={() => {
              setError("");
              setCooldownMs(0);
              setPin("");
              setPassword("");
              setMode((current) => current === "pin" ? "password" : "pin");
            }}
          >
            {mode === "pin" ? "Use Master Password" : "Use device PIN"}
          </button>
        ) : null}

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

      <UnauthenticatedResetDialog
        isOpen={isResetOpen}
        onClose={() => setIsResetOpen(false)}
        onReset={onReset}
      />
    </AuthLayout>
  );
}
