import { useMemo } from "react";
import { analyzePassword } from "../../../shared/security/passwordStrength";
export { analyzePassword, type StrengthResult } from "../../../shared/security/passwordStrength";

const SCORE_COLORS = ["#ef4444", "#f97316", "#eab308", "#22c55e", "#54f5ae"] as const;
const SCORE_WIDTHS = ["10%", "30%", "56%", "78%", "100%"] as const;

type PasswordStrengthMeterProps = {
  password: string;
};

export default function PasswordStrengthMeter({ password }: PasswordStrengthMeterProps) {
  const { score, label } = useMemo(() => analyzePassword(password), [password]);
  const color = SCORE_COLORS[score];

  return (
    <div
      className="pw-strength-meter"
      aria-label={`Password strength: ${label}`}
      aria-live="polite"
    >
      <div
        className="pw-strength-bar-track"
        role="progressbar"
        aria-valuenow={score}
        aria-valuemin={0}
        aria-valuemax={4}
        aria-label={label}
      >
        <div
          className="pw-strength-bar-fill"
          style={{ width: SCORE_WIDTHS[score], background: color }}
        />
      </div>
      <span className="pw-strength-label" style={{ color }}>
        {label}
      </span>
    </div>
  );
}
