export type AuthStatus =
  | "setup-required"
  | "locked"
  | "unlocked"
  | "data-error";

export type RecoveryKeyResult = {
  status: AuthStatus;
  recoveryKey: string;
};

export type RecoveryStatus = {
  configured: boolean;
};

export type PinStatus = {
  configured: boolean;
  unlockAvailable: boolean;
};

export class AuthClientError extends Error {
  readonly code: string;
  readonly retryAfterMs?: number;

  constructor(code: string, message: string, retryAfterMs?: number) {
    super(message);
    this.name = "AuthClientError";
    this.code = code;
    this.retryAfterMs = retryAfterMs;
  }
}
