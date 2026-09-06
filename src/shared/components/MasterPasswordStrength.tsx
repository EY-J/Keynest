import { masterPasswordStrength } from "../security/masterPasswordPolicy";
import "./MasterPasswordStrength.css";

export default function MasterPasswordStrength({ password }: { password: string }) {
  if (!password) return null;
  const strength = masterPasswordStrength(password);
  return (
    <div className="master-password-strength" data-strength={strength} role="status"
      aria-label={`Master Password strength: ${strength}`}>
      <span className="master-password-strength__track" aria-hidden="true">
        <span className="master-password-strength__fill" />
      </span>
      <span>{strength}</span>
    </div>
  );
}
