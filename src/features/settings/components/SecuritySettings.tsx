import { useRef, useState } from "react";
import { useSettings } from "../SettingsProvider";
import type { AutoLockSeconds, ClipboardClearSeconds } from "../types";
import AuthenticatedResetDialog from "./AuthenticatedResetDialog";
import ChangeMasterPasswordForm from "./ChangeMasterPasswordForm";

const SAVE_ERROR = "KeyNest could not save this security preference.";

type SecurityControl = "auto-lock" | "clipboard";

type SecuritySettingsProps = {
  onResetAuthenticated: (
    currentPassword: string,
    confirmation: "RESET KEYNEST",
  ) => Promise<void>;
};

export default function SecuritySettings({
  onResetAuthenticated,
}: SecuritySettingsProps) {
  const {
    settings,
    setAutoLockSeconds,
    setClipboardClearSeconds,
  } = useSettings();
  const requestIds = useRef<Record<SecurityControl, number>>({
    "auto-lock": 0,
    clipboard: 0,
  });
  const [isSaving, setIsSaving] = useState<Record<SecurityControl, boolean>>({
    "auto-lock": false,
    clipboard: false,
  });
  const [errors, setErrors] = useState({
    "auto-lock": "",
    clipboard: "",
  });
  const [isResetOpen, setIsResetOpen] = useState(false);

  async function save(
    control: SecurityControl,
    request: () => Promise<void>,
  ) {
    const requestId = requestIds.current[control] + 1;
    requestIds.current[control] = requestId;
    setErrors((current) => ({ ...current, [control]: "" }));
    setIsSaving((current) => ({ ...current, [control]: true }));
    try {
      await request();
    } catch {
      if (requestIds.current[control] === requestId) {
        setErrors((current) => ({ ...current, [control]: SAVE_ERROR }));
      }
    } finally {
      if (requestIds.current[control] === requestId) {
        setIsSaving((current) => ({ ...current, [control]: false }));
      }
    }
  }

  async function saveAutoLock(value: AutoLockSeconds) {
    await save("auto-lock", () => setAutoLockSeconds(value));
  }

  async function saveClipboardClear(value: ClipboardClearSeconds) {
    await save("clipboard", () => setClipboardClearSeconds(value));
  }

  return (
    <div className="security-settings">
      <div className="security-setting">
        <label htmlFor="auto-lock-seconds">Lock KeyNest after inactivity</label>
        <select
          id="auto-lock-seconds"
          value={settings.autoLockSeconds}
          disabled={isSaving["auto-lock"]}
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
          disabled={isSaving.clipboard}
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

      <ChangeMasterPasswordForm />

      <section className="security-reset" aria-labelledby="reset-keynest-settings-title">
        <h3 id="reset-keynest-settings-title">Reset KeyNest</h3>
        <p>Permanently erase this device&apos;s encrypted KeyNest data.</p>
        <button
          className="danger-button"
          type="button"
          onClick={() => setIsResetOpen(true)}
        >
          Reset KeyNest
        </button>
      </section>

      <AuthenticatedResetDialog
        isOpen={isResetOpen}
        onClose={() => setIsResetOpen(false)}
        onReset={onResetAuthenticated}
      />
    </div>
  );
}
