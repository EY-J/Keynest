import { analyzePassword } from "./passwordStrength";

export type MasterPasswordStrength = "Weak" | "Good" | "Strong";

// Master Password acceptance is separate from saved-credential presentation.
// Reuse the existing heuristic; Rust enforces the same boundary for all three
// creation flows. This estimate is not a measured entropy guarantee.
export function masterPasswordStrength(password: string): MasterPasswordStrength {
  if (Array.from(password).length < 12) return "Weak";
  const { score } = analyzePassword(password);
  return score < 2 ? "Weak" : score === 2 ? "Good" : "Strong";
}

export function validateMasterPassword(password: string, confirmation: string): string | null {
  if (Array.from(password).length < 12) return "Use at least 12 characters.";
  if (!confirmation || password !== confirmation) return "The passwords do not match.";
  if (masterPasswordStrength(password) === "Weak") return "Master Password is too weak.";
  return null;
}
