import { useEffect, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight, Star } from "lucide-react";
import ServiceLogo from "../../../components/ui/ServiceLogo";
import type { CredentialSummary } from "../types";

type CredentialCardStackProps = {
  credentials: CredentialSummary[];
  favoriteCredentialIds: ReadonlySet<string>;
  onOpenCredential(credentialId: string): void;
  onToggleFavorite(credentialId: string): void;
};

function getStackOffset(index: number, activeIndex: number, count: number) {
  const forward = (index - activeIndex + count) % count;
  const backward = forward - count;
  return Math.abs(forward) <= Math.abs(backward) ? forward : backward;
}

export default function CredentialCardStack({
  credentials,
  favoriteCredentialIds,
  onOpenCredential,
  onToggleFavorite,
}: CredentialCardStackProps) {
  const [focusedId, setFocusedId] = useState(credentials[0]?.id ?? "");

  useEffect(() => {
    if (!credentials.some((credential) => credential.id === focusedId)) {
      setFocusedId(credentials[0]?.id ?? "");
    }
  }, [credentials, focusedId]);

  const activeIndex = Math.max(
    0,
    credentials.findIndex((credential) => credential.id === focusedId),
  );

  const visibleCards = useMemo(
    () =>
      credentials
        .map((credential, index) => ({
          credential,
          index,
          offset: getStackOffset(index, activeIndex, credentials.length),
        }))
        .filter(({ offset }) => Math.abs(offset) <= 3),
    [activeIndex, credentials],
  );

  function moveFocus(direction: -1 | 1) {
    const nextIndex = (activeIndex + direction + credentials.length) % credentials.length;
    setFocusedId(credentials[nextIndex].id);
  }

  return (
    <section className="vault-stack-view" aria-label="Credentials in Card Stack View">
      <div className="vault-stack-stage">
        {credentials.length > 1 ? (
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
          {visibleCards.map(({ credential, index, offset }) => {
            const isFocused = index === activeIndex;
            const position = offset < 0 ? `n${Math.abs(offset)}` : `p${offset}`;
            const tag = credential.tags[0] || "Credential";
            const isFavorite = favoriteCredentialIds.has(credential.id);

            return (
              <article
                className={`vault-stack-card vault-stack-position-${position}${
                  isFocused ? " focused" : ""
                }`}
                key={credential.id}
              >
                <button
                  className="vault-stack-card-open"
                  type="button"
                  aria-label={`${isFocused ? "Open" : "Focus"} ${credential.name}`}
                  aria-current={isFocused ? "true" : undefined}
                  onClick={() => {
                    if (isFocused) onOpenCredential(credential.id);
                    else setFocusedId(credential.id);
                  }}
                >
                  <span className="vault-card-topline">
                    <ServiceLogo name={credential.name} website={credential.website} />
                  </span>
                  <span className="vault-card-copy">
                    <strong>{credential.name}</strong>
                    <span>{credential.username}</span>
                  </span>
                  <span className="vault-tag-chip">{tag}</span>
                </button>
                <button
                  className={`vault-favorite-button vault-card-favorite${
                    isFavorite ? " active" : ""
                  }`}
                  type="button"
                  aria-label={`${isFavorite ? "Remove" : "Add"} ${credential.name} ${
                    isFavorite ? "from" : "to"
                  } favorites`}
                  aria-pressed={isFavorite}
                  onClick={() => onToggleFavorite(credential.id)}
                >
                  <Star size={19} fill={isFavorite ? "currentColor" : "none"} aria-hidden="true" />
                </button>
              </article>
            );
          })}
        </div>

        {credentials.length > 1 ? (
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

      {credentials.length > 1 ? (
        <div className="vault-stack-pagination" aria-label="Choose credential">
          {credentials.map((credential, index) => (
            <button
              className={index === activeIndex ? "active" : ""}
              key={credential.id}
              type="button"
              aria-label={`Show ${credential.name}`}
              aria-current={index === activeIndex ? "true" : undefined}
              onClick={() => setFocusedId(credential.id)}
            />
          ))}
        </div>
      ) : null}

      <p className="vault-view-caption">Your credentials, always within reach.</p>
    </section>
  );
}
