import { useState } from "react";
import { useSettings } from "../SettingsProvider";
import type { ThemePreference } from "../types";

const SAVE_ERROR = "KeyNest could not save this appearance preference.";

const THEME_OPTIONS: Array<{ value: ThemePreference; label: string }> = [
  { value: "system", label: "System" },
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
];

export default function AppearanceSettings() {
  const { settings, setTheme } = useSettings();
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState("");

  async function updateTheme(theme: ThemePreference) {
    setError("");
    setIsSaving(true);
    try {
      await setTheme(theme);
    } catch {
      setError(SAVE_ERROR);
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <fieldset className="appearance-settings" disabled={isSaving}>
      <legend>Theme</legend>
      <p className="settings-help">Choose the appearance KeyNest uses on this device.</p>
      <div className="theme-options">
        {THEME_OPTIONS.map(({ value, label }) => (
          <label key={value} className="theme-option">
            <input
              type="radio"
              name="theme"
              value={value}
              checked={settings.theme === value}
              onChange={() => void updateTheme(value)}
            />
            {label}
          </label>
        ))}
      </div>
      {error ? <p role="alert">{error}</p> : null}
    </fieldset>
  );
}
