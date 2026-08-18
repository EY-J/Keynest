import { useState } from "react";
import { useSettings } from "../SettingsProvider";
import type { AutoLockSeconds, ClipboardClearSeconds } from "../types";

const SAVE_ERROR = "KeyNest could not save this security preference.";

type SavingControl = "auto-lock" | "clipboard" | null;

export default function SecuritySettings() {
  const {
    settings,
    setAutoLockSeconds,
    setClipboardClearSeconds,
  } = useSettings();
  const [savingControl, setSavingControl] = useState<SavingControl>(null);
  const [errors, setErrors] = useState({
    "auto-lock": "",
    clipboard: "",
  });

  async function saveAutoLock(value: AutoLockSeconds) {
    setErrors((current) => ({ ...current, "auto-lock": "" }));
    setSavingControl("auto-lock");
    try {
      await setAutoLockSeconds(value);
    } catch {
      setErrors((current) => ({ ...current, "auto-lock": SAVE_ERROR }));
    } finally {
      setSavingControl(null);
    }
  }

  async function saveClipboardClear(value: ClipboardClearSeconds) {
    setErrors((current) => ({ ...current, clipboard: "" }));
    setSavingControl("clipboard");
    try {
      await setClipboardClearSeconds(value);
    } catch {
      setErrors((current) => ({ ...current, clipboard: SAVE_ERROR }));
    } finally {
      setSavingControl(null);
    }
  }

  return (
    <div className="security-settings">
      <div className="security-setting">
        <label htmlFor="auto-lock-seconds">Lock KeyNest after inactivity</label>
        <select
          id="auto-lock-seconds"
          value={settings.autoLockSeconds}
          disabled={savingControl === "auto-lock"}
          onChange={(event) =>
            void saveAutoLock(Number(event.target.value) as AutoLockSeconds)
          }
        >
          <option value="60">1 minute</option>
          <option value="300">5 minutes</option>
          <option value="900">15 minutes</option>
          <option value="1800">30 minutes</option>
        </select>
        {errors["auto-lock"] ? (
          <p role="alert">{errors["auto-lock"]}</p>
        ) : null}
      </div>

      <div className="security-setting">
        <label htmlFor="clipboard-clear-seconds">Clear clipboard after</label>
        <select
          id="clipboard-clear-seconds"
          value={settings.clipboardClearSeconds}
          disabled={savingControl === "clipboard"}
          onChange={(event) =>
            void saveClipboardClear(
              Number(event.target.value) as ClipboardClearSeconds,
            )
          }
        >
          <option value="10">10 seconds</option>
          <option value="30">30 seconds</option>
          <option value="60">60 seconds</option>
        </select>
        {errors.clipboard ? <p role="alert">{errors.clipboard}</p> : null}
      </div>

      <div className="security-setting">
        <p>Lock when Windows sleeps</p>
        <p>Enabled</p>
      </div>
    </div>
  );
}
