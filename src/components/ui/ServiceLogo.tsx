import type { CSSProperties } from "react";
import { getServiceIdentity } from "../../utils/serviceIdentity";

type ServiceLogoProps = {
  name: string;
  website?: string | null;
  size?: "small" | "medium" | "large";
  decorative?: boolean;
  className?: string;
};

function logoHue(name: string) {
  return [...name].reduce((total, character) => total + character.charCodeAt(0), 0) % 360;
}

export default function ServiceLogo({
  name,
  website,
  size = "medium",
  decorative = true,
  className = "",
}: ServiceLogoProps) {
  const service = getServiceIdentity(website);
  const label = service?.label ?? `${name || "Credential"} monogram`;
  const style = service ? undefined : { "--vault-logo-hue": logoHue(name) };

  return (
    <span
      className={`service-icon service-icon--${size} service-icon--${service ? "known" : "fallback"} ${className}`.trim()}
      style={style as CSSProperties}
      role={!decorative && !service ? "img" : undefined}
      aria-label={!decorative && !service ? label : undefined}
      aria-hidden={decorative || undefined}
      title={decorative ? undefined : label}
    >
      {service ? (
        <img className="service-icon__brand" src={service.icon} alt={decorative ? "" : label} />
      ) : (
        name.trim().charAt(0).toLocaleUpperCase() || "?"
      )}
    </span>
  );
}
