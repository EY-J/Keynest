import { type FormEvent, useState } from "react";
import { authClient } from "../authClient";
import { AuthClientError } from "../types";
import AuthLayout from "./AuthLayout";
import PasswordField from "./PasswordField";

type SetupScreenProps = {
  onCreated: () => void;
};

export default function SetupScreen({ onCreated }: SetupScreenProps) {
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError("");

    if (Array.from(password).length < 12) {
      setError("Use at least 12 characters.");
      return;
    }
    if (password !== confirmation) {
      setError("The passwords do not match.");
      return;
    }

    setIsSubmitting(true);
    try {
      const status = await authClient.createMasterPassword(password);
      setPassword("");
      setConfirmation("");
      if (status !== "unlocked") {
        setError("KeyNest did not unlock after creating the master password.");
        return;
      }
      onCreated();
    } catch (requestError) {
      setPassword("");
      setConfirmation("");
      setError(
        requestError instanceof AuthClientError
          ? requestError.message
          : "KeyNest could not create the master password.",
      );
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <AuthLayout
      eyebrow="FIRST-TIME SETUP"
      title="Create your master password"
      description="This password unlocks your encrypted KeyNest data on this device."
    >
      <form className="auth-form" onSubmit={(event) => void submit(event)}>
        <PasswordField
          label="Master password"
          value={password}
          onChange={setPassword}
          autoComplete="new-password"
          autoFocus
          disabled={isSubmitting}
        />
        <PasswordField
          label="Confirm master password"
          value={confirmation}
          onChange={setConfirmation}
          autoComplete="new-password"
          disabled={isSubmitting}
        />

        <p className="auth-requirement">Use at least 12 characters.</p>
        <div className="auth-warning">
          <strong>No password recovery</strong>
          <span>
            If you forget this password, your encrypted KeyNest data cannot be
            recovered.
          </span>
        </div>

        {error ? (
          <p className="auth-error" role="alert">
            {error}
          </p>
        ) : null}

        <button className="primary-button auth-submit" disabled={isSubmitting}>
          {isSubmitting ? "Creating encrypted vault…" : "Create Master Password"}
        </button>
      </form>
    </AuthLayout>
  );
}
