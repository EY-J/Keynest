import { useState } from "react";
import { Power } from "lucide-react";
import { useSettings } from "../SettingsProvider";
import SettingsRow from "./SettingsRow";

const SAVE_ERROR = "KeyNest could not save this general preference.";

export default function GeneralSettings() {
  const { settings, setLaunchAtStartup } = useSettings();
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState("");

  async function updateLaunchAtStartup() {
    setError("");
    setIsSaving(true);
    try {
      await setLaunchAtStartup(!settings.launchAtStartup);
    } catch {
      setError(SAVE_ERROR);
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="settings-row-list settings-section-body">
      <SettingsRow
        icon={Power}
        title="Launch at Startup"
        description="Open KeyNest minimized and locked when Windows starts."
      >
        <div className="settings-control-stack">
          <label className="settings-switch">
            <input
              type="checkbox"
              checked={settings.launchAtStartup}
              disabled={isSaving}
              aria-label="Launch KeyNest at startup"
              onChange={() => void updateLaunchAtStartup()}
            />
            <span aria-hidden="true" />
          </label>
          {error ? (
            <span className="settings-inline-error" role="alert">{error}</span>
          ) : null}
        </div>
      </SettingsRow>
    </div>
  );
}
