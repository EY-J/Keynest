import { type FormEvent, useState } from "react";
import { authClient } from "../../auth/authClient";
import { AuthClientError } from "../../auth/types";
import PasswordField from "../../auth/components/PasswordField";

const SUCCESS_MESSAGE =
  "Master password changed. Your new password will be required the next time KeyNest locks.";

export default function ChangeMasterPasswordForm() {
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError("");
    setSuccess("");

    if (!currentPassword) {
      setError("Enter your current master password.");
      return;
    }
    if (Array.from(newPassword).length < 12) {
      setError("Use at least 12 characters.");
      return;
    }
    if (newPassword !== confirmation) {
      setError("The passwords do not match.");
      return;
    }

    setIsSubmitting(true);
    try {
      const status = await authClient.changeMasterPassword(
        currentPassword,
        newPassword,
      );
      if (status !== "unlocked") {
        setError("KeyNest could not confirm that the master password was changed.");
        return;
      }
      setSuccess(SUCCESS_MESSAGE);
    } catch (requestError) {
      setError(
        requestError instanceof AuthClientError
          ? requestError.message
          : "KeyNest could not change the master password.",
      );
    } finally {
      setCurrentPassword("");
      setNewPassword("");
      setConfirmation("");
      setIsSubmitting(false);
    }
  }

  return (
    <section className="security-password-change" aria-labelledby="change-master-password-title">
      <h3 id="change-master-password-title">Change master password</h3>
      <form className="auth-form" onSubmit={(event) => void submit(event)}>
        <PasswordField
          label="Current master password"
          value={currentPassword}
          onChange={setCurrentPassword}
          autoComplete="current-password"
          disabled={isSubmitting}
        />
        <PasswordField
          label="New master password"
          value={newPassword}
          onChange={setNewPassword}
          autoComplete="new-password"
          disabled={isSubmitting}
        />
        <PasswordField
          label="Confirm new master password"
          value={confirmation}
          onChange={setConfirmation}
          autoComplete="new-password"
          disabled={isSubmitting}
        />
        <p className="auth-requirement">Use at least 12 characters.</p>
        {error ? <p className="auth-error" role="alert">{error}</p> : null}
        {success ? <p className="security-success" role="status">{success}</p> : null}
        <button className="primary-button" disabled={isSubmitting}>
          {isSubmitting ? "Changing…" : "Change master password"}
        </button>
      </form>
    </section>
  );
}
