import { useEffect, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight, Star } from "lucide-react";
import ServiceIcon from "../../../shared/components/ServiceIcon";
import type { VaultRecordSummary } from "../types";

type VaultCardStackProps = {
  records: VaultRecordSummary[];
  favoriteRecordIds: ReadonlySet<string>;
  onOpenRecord(recordId: string): void;
  onToggleFavorite(recordId: string): void;
};

function getStackOffset(index: number, activeIndex: number, count: number) {
  const forward = (index - activeIndex + count) % count;
  const backward = forward - count;
  return Math.abs(forward) <= Math.abs(backward) ? forward : backward;
}

export default function VaultCardStack({
  records,
  favoriteRecordIds,
  onOpenRecord,
  onToggleFavorite,
}: VaultCardStackProps) {
  const [focusedId, setFocusedId] = useState(records[0]?.id ?? "");

  useEffect(() => {
    if (!records.some((record) => record.id === focusedId)) {
      setFocusedId(records[0]?.id ?? "");
    }
  }, [focusedId, records]);

  const activeIndex = Math.max(
    0,
    records.findIndex((record) => record.id === focusedId),
  );

  const visibleCards = useMemo(
    () =>
      records
        .map((record, index) => ({
          record,
          index,
          offset: getStackOffset(index, activeIndex, records.length),
        }))
        .filter(({ offset }) => Math.abs(offset) <= 3),
    [activeIndex, records],
  );

  function moveFocus(direction: -1 | 1) {
    const nextIndex = (activeIndex + direction + records.length) % records.length;
    setFocusedId(records[nextIndex].id);
  }

  return (
    <section className="vault-stack-view" aria-label="Credentials in Card Stack View">
      <div className="vault-stack-stage">
        {records.length > 1 ? (
          <button
            className="vault-stack-arrow vault-stack-arrow-left"
            type="button"
            aria-label="Previous credential"
            onClick={() => moveFocus(-1)}
          >
            <ChevronLeft size={20} aria-hidden="true" />
          </button>
        ) : null}

        <div className="vault-stack-cards">
          {visibleCards.map(({ record, index, offset }) => {
            const isFocused = index === activeIndex;
            const position = offset < 0 ? `n${Math.abs(offset)}` : `p${offset}`;
            const tag = record.tags[0] || "Credential";
            const isFavorite = favoriteRecordIds.has(record.id);

            return (
              <article
                className={`vault-stack-card vault-stack-position-${position}${
                  isFocused ? " focused" : ""
                }`}
                key={record.id}
              >
                <button
                  className="vault-stack-card-open"
                  type="button"
                  aria-label={`${isFocused ? "Open" : "Focus"} ${record.name}`}
                  aria-current={isFocused ? "true" : undefined}
                  onClick={() => {
                    if (isFocused) onOpenRecord(record.id);
                    else setFocusedId(record.id);
                  }}
                >
                  <span className="vault-card-topline">
                    <ServiceIcon name={record.name} website={record.website} />
                  </span>
                  <span className="vault-card-copy">
                    <strong>{record.name}</strong>
                    <span>{record.username}</span>
                  </span>
                  <span className="vault-tag-chip">{tag}</span>
                </button>
                <button
                  className={`vault-favorite-button vault-card-favorite${
                    isFavorite ? " active" : ""
                  }`}
                  type="button"
                  aria-label={`${isFavorite ? "Remove" : "Add"} ${record.name} ${
                    isFavorite ? "from" : "to"
                  } favorites`}
                  aria-pressed={isFavorite}
                  onClick={() => onToggleFavorite(record.id)}
                >
                  <Star size={19} fill={isFavorite ? "currentColor" : "none"} aria-hidden="true" />
                </button>
              </article>
            );
          })}
        </div>

        {records.length > 1 ? (
          <button
            className="vault-stack-arrow vault-stack-arrow-right"
            type="button"
            aria-label="Next credential"
            onClick={() => moveFocus(1)}
          >
            <ChevronRight size={20} aria-hidden="true" />
          </button>
        ) : null}
      </div>

      {records.length > 1 ? (
        <div className="vault-stack-pagination" aria-label="Choose credential">
          {records.map((record, index) => (
            <button
              className={index === activeIndex ? "active" : ""}
              key={record.id}
              type="button"
              aria-label={`Show ${record.name}`}
              aria-current={index === activeIndex ? "true" : undefined}
              onClick={() => setFocusedId(record.id)}
            />
          ))}
        </div>
      ) : null}

      <p className="vault-view-caption">Your credentials, always within reach.</p>
    </section>
  );
}
