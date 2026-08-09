export type AuthStatus =
  | "setup-required"
  | "locked"
  | "unlocked"
  | "data-error";

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
