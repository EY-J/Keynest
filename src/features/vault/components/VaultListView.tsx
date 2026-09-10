import { Star } from "lucide-react";
import ServiceIcon from "../../../shared/components/ServiceIcon";
import type { VaultRecordSummary } from "../types";

type VaultListViewProps = {
  records: VaultRecordSummary[];
  favoriteRecordIds: ReadonlySet<string>;
  onOpenRecord(recordId: string): void;
  onToggleFavorite(recordId: string): void;
};

const updatedDateFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
  year: "numeric",
});

function formatUpdatedDate(timestamp: number) {
  return updatedDateFormatter.format(new Date(timestamp));
}

export default function VaultListView({
  records,
  favoriteRecordIds,
  onOpenRecord,
  onToggleFavorite,
}: VaultListViewProps) {
  return (
    <section className="vault-list-view" aria-label="Credentials in List View">
      <div className="vault-list-header" aria-hidden="true">
        <div className="vault-list-header-main">
          <span>Name</span>
          <span>Username / Email</span>
          <span>Tag</span>
          <span>Last Updated</span>
        </div>
        <span className="vault-list-favorite-heading">Favorite</span>
      </div>
      <div className="vault-list-rows">
        {records.map((record) => {
          const isFavorite = favoriteRecordIds.has(record.id);

          return (
            <div
              className="vault-list-row"
              key={record.id}
            >
              <button
                className="vault-list-row-main"
                type="button"
                aria-label={`Open ${record.name}`}
                onClick={() => onOpenRecord(record.id)}
              >
                <span className="vault-list-name">
                  <ServiceIcon name={record.name} website={record.website} size="small" />
                  <strong>{record.name}</strong>
                </span>
                <span className="vault-list-username">{record.username}</span>
                <span>
                  <span className="vault-tag-chip">{record.tags[0] || "Credential"}</span>
                </span>
                <time dateTime={new Date(record.updatedAtMs).toISOString()}>
                  {formatUpdatedDate(record.updatedAtMs)}
                </time>
              </button>
              <button
                className={`vault-favorite-button vault-list-favorite${
                  isFavorite ? " active" : ""
                }`}
                type="button"
                aria-label={`${isFavorite ? "Remove" : "Add"} ${record.name} ${
                  isFavorite ? "from" : "to"
                } favorites`}
                aria-pressed={isFavorite}
                onClick={() => onToggleFavorite(record.id)}
              >
                <Star size={17} fill={isFavorite ? "currentColor" : "none"} aria-hidden="true" />
              </button>
            </div>
          );
        })}
      </div>
    </section>
  );
}
