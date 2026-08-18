import { useState } from "react";
import { useSettings } from "../SettingsProvider";

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
    <div className="general-settings">
      <label className="settings-toggle" htmlFor="launch-at-startup">
        <input
          id="launch-at-startup"
          type="checkbox"
          checked={settings.launchAtStartup}
          disabled={isSaving}
          onChange={() => void updateLaunchAtStartup()}
        />
        <span>Launch KeyNest at startup</span>
      </label>
      <p className="settings-help">
        When enabled, KeyNest starts minimized and locked so your encrypted data
        stays private until you unlock it.
      </p>
      {error ? <p role="alert">{error}</p> : null}
    </div>
  );
}
