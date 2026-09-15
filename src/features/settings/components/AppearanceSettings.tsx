import { useState } from "react";
import { Palette } from "lucide-react";
import { useSettings } from "../SettingsProvider";
import type { ThemePreference } from "../types";
import SettingsRow from "./SettingsRow";

const SAVE_ERROR = "KeyNest could not save this appearance preference.";

const THEME_OPTIONS: Array<{
  value: ThemePreference;
  label: string;
  disabled?: boolean;
}> = [
  { value: "system", label: "System" },
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light", disabled: true },
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
    <div className="settings-row-list settings-section-body">
      <SettingsRow
        icon={Palette}
        title="Theme"
      >
        <fieldset className="theme-segmented" disabled={isSaving}>
          <legend className="sr-only">Theme preference</legend>
          {THEME_OPTIONS.map(({ value, label, disabled = false }) => (
            <label
              key={value}
              className={[
                settings.theme === value ? "selected" : "",
                disabled ? "disabled" : "",
              ].filter(Boolean).join(" ")}
            >
              <input
                type="radio"
                name="theme"
                value={value}
                checked={settings.theme === value}
                disabled={disabled}
                onChange={() => void updateTheme(value)}
              />
              <span>{label}</span>
            </label>
          ))}
        </fieldset>
        {error ? (
          <span className="settings-inline-error" role="alert">{error}</span>
        ) : null}
      </SettingsRow>
    </div>
  );
}
