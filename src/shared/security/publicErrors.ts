// Never display transport-provided messages, paths, stacks or serialized payloads.
const messages: Record<string, string> = {
  "internal-error": "KeyNest could not complete the security request.",
  "password-too-short": "Use at least 12 characters.",
  "password-too-weak": "Master Password is too weak.",
  "already-initialized": "KeyNest already has a master password.",
  "not-initialized": "Create a master password before unlocking KeyNest.",
  "invalid-credentials": "The master password is incorrect.",
  "recovery-not-configured": "This KeyNest profile does not have a Recovery Key yet.",
  "invalid-recovery-key": "The Recovery Key is incorrect.",
  throttled: "Wait a moment before trying again.",
  "invalid-reset-confirmation": "Type RESET KEYNEST exactly to confirm.",
  unauthorized: "KeyNest is locked.",
  "data-error": "KeyNest could not verify your local data.",
  "local-data-error": "KeyNest could not access its encrypted local data.",
  "invalid-auto-lock": "Choose a supported automatic lock duration.",
  "invalid-clipboard-duration": "Choose a supported clipboard clearing duration.",
  "invalid-theme": "Choose System, Dark, or Light.",
  "settings-error": "KeyNest could not save the settings change.",
  "startup-error": "KeyNest could not update or confirm its startup setting.",
  "clipboard-error": "KeyNest could not safely clear its clipboard content.",
  "clipboard-copy-error": "KeyNest could not copy the secret.",
  "folder-open-error": "KeyNest could not open its data folder.",
  "invalid-vault-name": "Enter a credential name.",
  "invalid-vault-username": "Enter a credential username.",
  "invalid-vault-password": "Enter a credential password.",
  "invalid-vault-website": "Enter a valid credential website.",
  "invalid-vault-tags": "Check the credential tags.",
  "vault-record-not-found": "The credential was not found.",
  "vault-data-error": "KeyNest could not verify your vault data.",
  "vault-entropy-error": "KeyNest could not generate secure vault data.",
  "vault-storage-error": "KeyNest could not access its encrypted vault data.",
};

export function publicError(error: unknown) {
  const value = typeof error === "object" && error !== null
    ? error as Record<string, unknown> : {};
  const code = typeof value.code === "string" && Object.prototype.hasOwnProperty.call(messages, value.code)
    ? value.code : "unknown-error";
  const retryAfterMs = code === "throttled" && typeof value.retryAfterMs === "number"
    && Number.isFinite(value.retryAfterMs)
    ? Math.min(30_000, Math.max(0, Math.ceil(value.retryAfterMs))) : undefined;
  return { code, message: messages[code] ?? "KeyNest could not complete the request.", retryAfterMs };
}
