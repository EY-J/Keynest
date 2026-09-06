import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

type SettingsRowProps = {
  icon: LucideIcon;
  title: string;
  description?: string;
  children?: ReactNode;
  className?: string;
};

export default function SettingsRow({
  icon: Icon,
  title,
  description,
  children,
  className = "",
}: SettingsRowProps) {
  return (
    <div className={`settings-row ${className}`.trim()}>
      <span className="settings-row-icon" aria-hidden="true">
        <Icon size={19} strokeWidth={1.8} />
      </span>
      <div className="settings-row-copy">
        <h3>{title}</h3>
        {description ? <p>{description}</p> : null}
      </div>
      {children ? <div className="settings-row-action">{children}</div> : null}
    </div>
  );
}
