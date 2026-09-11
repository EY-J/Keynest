import { Star } from "lucide-react";
import ServiceLogo from "../../../components/ui/ServiceLogo";
import type { CredentialSummary } from "../types";

type CredentialListProps = {
  credentials: CredentialSummary[];
  favoriteCredentialIds: ReadonlySet<string>;
  onOpenCredential(credentialId: string): void;
  onToggleFavorite(credentialId: string): void;
};

const updatedDateFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
  year: "numeric",
});

function formatUpdatedDate(timestamp: number) {
  return updatedDateFormatter.format(new Date(timestamp));
}

export default function CredentialList({
  credentials,
  favoriteCredentialIds,
  onOpenCredential,
  onToggleFavorite,
}: CredentialListProps) {
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
        {credentials.map((credential) => {
          const isFavorite = favoriteCredentialIds.has(credential.id);

          return (
            <div
              className="vault-list-row"
              key={credential.id}
            >
              <button
                className="vault-list-row-main"
                type="button"
                aria-label={`Open ${credential.name}`}
                onClick={() => onOpenCredential(credential.id)}
              >
                <span className="vault-list-name">
                  <ServiceLogo name={credential.name} website={credential.website} size="small" />
                  <strong>{credential.name}</strong>
                </span>
                <span className="vault-list-username">{credential.username}</span>
                <span>
                  <span className="vault-tag-chip">{credential.tags[0] || "Credential"}</span>
                </span>
                <time dateTime={new Date(credential.updatedAtMs).toISOString()}>
                  {formatUpdatedDate(credential.updatedAtMs)}
                </time>
              </button>
              <button
                className={`vault-favorite-button vault-list-favorite${
                  isFavorite ? " active" : ""
                }`}
                type="button"
                aria-label={`${isFavorite ? "Remove" : "Add"} ${credential.name} ${
                  isFavorite ? "from" : "to"
                } favorites`}
                aria-pressed={isFavorite}
                onClick={() => onToggleFavorite(credential.id)}
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
