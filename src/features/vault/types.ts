export type CredentialInput = {
  name: string;
  username: string;
  password: string;
  website: string | null;
  allowedLoginHosts: string[];
  tags: string[];
};

export type CredentialSummary = {
  id: string;
  name: string;
  username: string;
  website: string | null;
  allowedLoginHosts: string[];
  tags: string[];
  createdAtMs: number;
  updatedAtMs: number;
};

export type Credential = {
  id: string;
  name: string;
  username: string;
  password: string;
  website: string | null;
  allowedLoginHosts: string[];
  tags: string[];
  createdAtMs: number;
  updatedAtMs: number;
};

export class VaultClientError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "VaultClientError";
    this.code = code;
  }
}
