import { useRef, useState } from "react";
import { Clipboard, Lock, Monitor, Trash2 } from "lucide-react";
import Select from "../../../components/ui/Select";
import { useSettings } from "../SettingsProvider";
import type { AutoLockSeconds, ClipboardClearSeconds } from "../types";
import AuthenticatedResetDialog from "./AuthenticatedResetDialog";
import ChangeMasterPasswordForm from "./ChangeMasterPasswordForm";
import RecoverySettings from "./RecoverySettings";
import SettingsRow from "./SettingsRow";

const SAVE_ERROR = "KeyNest could not save this security preference.";

type SecurityControl = "auto-lock" | "clipboard" | "sleep-lock";

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
    setLockOnSleep,
  } = useSettings();
  const requestIds = useRef<Record<SecurityControl, number>>({
    "auto-lock": 0,
    clipboard: 0,
    "sleep-lock": 0,
  });
  const [isSaving, setIsSaving] = useState<Record<SecurityControl, boolean>>({
    "auto-lock": false,
    clipboard: false,
    "sleep-lock": false,
  });
  const [errors, setErrors] = useState({
    "auto-lock": "",
    clipboard: "",
    "sleep-lock": "",
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

  async function saveLockOnSleep(enabled: boolean) {
    await save("sleep-lock", () => setLockOnSleep(enabled));
  }

  return (
    <div className="security-settings">
      <div className="settings-row-list">
        <SettingsRow
          icon={Lock}
          title="Auto Lock"
        >
          <div className="settings-control-stack">
            <label className="sr-only" htmlFor="auto-lock-seconds">
              Auto Lock duration
            </label>
            <Select
              id="auto-lock-seconds"
              className="settings-time-select"
              menuClassName="settings-time-select-menu"
              value={String(settings.autoLockSeconds)}
              ariaLabel="Auto Lock duration"
              disabled={isSaving["auto-lock"]}
              onChange={(value) =>
                void saveAutoLock(Number(value) as AutoLockSeconds)
              }
              options={[
                { value: "60", label: "1 minute" },
                { value: "300", label: "5 minutes" },
                { value: "900", label: "15 minutes" },
                { value: "1800", label: "30 minutes" },
              ]}
            />
            {errors["auto-lock"] ? (
              <span className="settings-inline-error" role="alert">
                {errors["auto-lock"]}
              </span>
            ) : null}
          </div>
        </SettingsRow>

        <SettingsRow
          icon={Clipboard}
          title="Clipboard"
        >
          <div className="settings-control-stack">
            <label className="sr-only" htmlFor="clipboard-clear-seconds">
              Clipboard clearing delay
            </label>
            <Select
              id="clipboard-clear-seconds"
              className="settings-time-select"
              menuClassName="settings-time-select-menu"
              value={String(settings.clipboardClearSeconds)}
              ariaLabel="Clipboard clearing delay"
              disabled={isSaving.clipboard}
              onChange={(value) =>
                void saveClipboardClear(
                  Number(value) as ClipboardClearSeconds,
                )
              }
              options={[
                { value: "10", label: "10 seconds" },
                { value: "30", label: "30 seconds" },
                { value: "60", label: "60 seconds" },
              ]}
            />
            {errors.clipboard ? (
              <span className="settings-inline-error" role="alert">
                {errors.clipboard}
              </span>
            ) : null}
          </div>
        </SettingsRow>

        <SettingsRow
          icon={Monitor}
          title="Windows Sleep"
        >
          <div className="settings-control-stack">
            <label className="settings-switch">
              <input
                type="checkbox"
                checked={settings.lockOnSleep}
                disabled={isSaving["sleep-lock"]}
                onChange={(event) => void saveLockOnSleep(event.target.checked)}
                aria-label="Lock KeyNest when Windows sleeps"
              />
              <span aria-hidden="true" />
            </label>
            {errors["sleep-lock"] ? (
              <span className="settings-inline-error" role="alert">
                {errors["sleep-lock"]}
              </span>
            ) : null}
          </div>
        </SettingsRow>
      </div>

      <p className="settings-group-label">RECOVERY</p>
      <div className="settings-row-list">
        <RecoverySettings />
      </div>

      <p className="settings-group-label">ACCESS</p>
      <div className="settings-row-list">
        <ChangeMasterPasswordForm />
      </div>

      <p className="settings-group-label settings-danger-label">DANGER ZONE</p>
      <div className="settings-row-list">
        <SettingsRow
          icon={Trash2}
          title="Reset KeyNest"
          className="settings-row-danger"
        >
          <button
            className="danger-button compact-button"
            type="button"
            onClick={() => setIsResetOpen(true)}
          >
            Reset
          </button>
        </SettingsRow>
      </div>

      <AuthenticatedResetDialog
        isOpen={isResetOpen}
        onClose={() => setIsResetOpen(false)}
        onReset={onResetAuthenticated}
      />
    </div>
  );
}
