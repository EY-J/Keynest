const FAVORITE_RECORD_IDS_KEY = "keynest:vault-favorites:v1";
const MAX_FAVORITE_RECORDS = 10_000;
const MAX_RECORD_ID_LENGTH = 128;

export function loadFavoriteRecordIds(): Set<string> {
  if (typeof window === "undefined") return new Set();

  try {
    const stored = JSON.parse(window.localStorage.getItem(FAVORITE_RECORD_IDS_KEY) ?? "[]");
    if (!Array.isArray(stored)) return new Set();

    return new Set(
      stored
        .filter(
          (value): value is string =>
            typeof value === "string" &&
            value.length > 0 &&
            value.length <= MAX_RECORD_ID_LENGTH,
        )
        .slice(0, MAX_FAVORITE_RECORDS),
    );
  } catch {
    return new Set();
  }
}

export function saveFavoriteRecordIds(recordIds: ReadonlySet<string>) {
  try {
    window.localStorage.setItem(
      FAVORITE_RECORD_IDS_KEY,
      JSON.stringify([...recordIds].slice(0, MAX_FAVORITE_RECORDS)),
    );
  } catch {
    // Favorites remain usable in memory when preference storage is unavailable.
  }
}

export function toggledFavoriteRecordIds(
  current: ReadonlySet<string>,
  recordId: string,
) {
  const next = new Set(current);
  if (next.has(recordId)) next.delete(recordId);
  else next.add(recordId);
  return next;
}
