import { useMemo } from "react";

export interface StrengthResult {
  score: 0 | 1 | 2 | 3 | 4;
  label: string;
  patterns: string[];
  suggestions: string[];
}

const COMMON_PASSWORDS = [
  "password",
  "passw0rd",
  "letmein",
  "qwerty",
  "admin",
  "welcome",
  "iloveyou",
  "abc123",
] as const;

const SCORE_COLORS = ["#ef4444", "#f97316", "#eab308", "#22c55e", "#54f5ae"] as const;
const SCORE_WIDTHS = ["10%", "30%", "56%", "78%", "100%"] as const;

export function analyzePassword(password: string): StrengthResult {
  if (!password) {
    return { score: 0, label: "Very Weak", patterns: [], suggestions: [] };
  }

  const hasLower = /[a-z]/.test(password);
  const hasUpper = /[A-Z]/.test(password);
  const hasDigit = /\d/.test(password);
  const hasSymbol = /[^a-zA-Z0-9]/.test(password);
  const lowerPassword = password.toLocaleLowerCase();
  const patterns: string[] = [];
  let penalty = 0;
  let poolSize = 0;

  if (hasLower) poolSize += 26;
  if (hasUpper) poolSize += 26;
  if (hasDigit) poolSize += 10;
  if (hasSymbol) poolSize += 32;

  const commonMatch = COMMON_PASSWORDS.find((candidate) =>
    lowerPassword.includes(candidate),
  );
  if (commonMatch) {
    patterns.push(`Common password pattern "${commonMatch}"`);
    penalty += 30;
  }
  if (/0123|1234|2345|3456|4567|5678|6789|9876|8765|7654|6543|5432|4321/.test(lowerPassword)) {
    patterns.push("Sequential digits");
    penalty += 12;
  }
  if (/qwert|asdf|zxcv/.test(lowerPassword)) {
    patterns.push("Keyboard pattern");
    penalty += 18;
  }
  if (/(.)\1{2,}/.test(password)) {
    patterns.push("Repeated characters");
    penalty += 12;
  }
  if (/(19|20)\d{2}|\b\d{1,2}[/-]\d{1,2}(?:[/-]\d{2,4})?\b/.test(password)) {
    patterns.push("Date-like pattern");
    penalty += 10;
  }

  const estimatedEntropy = password.length * Math.log2(Math.max(poolSize, 1));
  const effectiveEntropy = Math.max(0, estimatedEntropy - penalty);

  let score: StrengthResult["score"];
  let label: string;
  if (effectiveEntropy < 25) {
    score = 0;
    label = "Very Weak";
  } else if (effectiveEntropy < 40) {
    score = 1;
    label = "Weak";
  } else if (effectiveEntropy < 55) {
    score = 2;
    label = "Fair";
  } else if (effectiveEntropy < 70) {
    score = 3;
    label = "Strong";
  } else {
    score = 4;
    label = "Very Strong";
  }

  const suggestions: string[] = [];
  if (password.length < 12) suggestions.push("Use at least 12 characters");
  if (!hasUpper) suggestions.push("Add uppercase letters");
  if (!hasDigit) suggestions.push("Add numbers");
  if (!hasSymbol) suggestions.push("Add symbols");
  if (patterns.length > 0) suggestions.push("Avoid predictable patterns");

  return { score, label, patterns, suggestions };
}

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
