import { invoke } from "@tauri-apps/api/core";
import { publicError } from "../../shared/security/publicErrors";
import {
  SettingsClientError,
  type AutoLockSeconds,
  type ClipboardClearSeconds,
  type SettingsSnapshot,
  type ThemePreference,
} from "./types";

type InvokeArguments = Record<string, unknown>;

async function invokeSettings<T>(
  command: string,
  argumentsValue?: InvokeArguments,
): Promise<T> {
  try {
    if (argumentsValue === undefined) {
      return await invoke<T>(command);
    }
    return await invoke<T>(command, argumentsValue);
  } catch (error) {
    throw normalizeSettingsError(error);
  }
}

function normalizeSettingsError(error: unknown): SettingsClientError {
  const { code, message } = publicError(error);
  return new SettingsClientError(code, message);
}

export const settingsClient = {
  getSettings: () => invokeSettings<SettingsSnapshot>("get_settings"),

  setAutoLockSeconds: (seconds: AutoLockSeconds) =>
    invokeSettings<SettingsSnapshot>("set_auto_lock_seconds", { seconds }),

  setClipboardClearSeconds: (seconds: ClipboardClearSeconds) =>
    invokeSettings<SettingsSnapshot>("set_clipboard_clear_seconds", { seconds }),

  setTheme: (theme: ThemePreference) =>
    invokeSettings<SettingsSnapshot>("set_theme", { theme }),

  setLockOnSleep: (enabled: boolean) =>
    invokeSettings<SettingsSnapshot>("set_lock_on_sleep",{ enabled }),

  setLaunchAtStartup: (enabled: boolean) =>
    invokeSettings<SettingsSnapshot>("set_launch_at_startup", { enabled }),

  recordActivity: () => invokeSettings<void>("record_activity"),

  openDataFolder: () => invokeSettings<void>("open_keynest_data_folder"),
};
