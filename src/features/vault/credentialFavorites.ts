const FAVORITE_CREDENTIAL_IDS_KEY = "keynest:vault-favorites:v1";
const MAX_FAVORITE_CREDENTIALS = 10_000;
const MAX_CREDENTIAL_ID_LENGTH = 128;

export function loadFavoriteCredentialIds(): Set<string> {
  if (typeof window === "undefined") return new Set();

  try {
    const stored = JSON.parse(window.localStorage.getItem(FAVORITE_CREDENTIAL_IDS_KEY) ?? "[]");
    if (!Array.isArray(stored)) return new Set();

    return new Set(
      stored
        .filter(
          (value): value is string =>
            typeof value === "string" &&
            value.length > 0 &&
            value.length <= MAX_CREDENTIAL_ID_LENGTH,
        )
        .slice(0, MAX_FAVORITE_CREDENTIALS),
    );
  } catch {
    return new Set();
  }
}

export function saveFavoriteCredentialIds(credentialIds: ReadonlySet<string>) {
  try {
    window.localStorage.setItem(
      FAVORITE_CREDENTIAL_IDS_KEY,
      JSON.stringify([...credentialIds].slice(0, MAX_FAVORITE_CREDENTIALS)),
    );
  } catch {
    // Favorites remain usable in memory when preference storage is unavailable.
  }
}

export function toggleFavoriteCredentialId(
  current: ReadonlySet<string>,
  credentialId: string,
) {
  const next = new Set(current);
  if (next.has(credentialId)) next.delete(credentialId);
  else next.add(credentialId);
  return next;
}
