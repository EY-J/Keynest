export type VaultRecordInput = {
  name: string;
  username: string;
  password: string;
  website: string | null;
  category: string;
  tags: string[];
};

export type VaultRecordSummary = {
  id: string;
  name: string;
  username: string;
  website: string | null;
  category: string;
  tags: string[];
  createdAtMs: number;
  updatedAtMs: number;
};

export type VaultRecord = {
  id: string;
  name: string;
  username: string;
  password: string;
  website: string | null;
  category: string;
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
