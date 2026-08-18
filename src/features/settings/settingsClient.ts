import { invoke } from "@tauri-apps/api/core";
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
  if (isRecord(error)) {
    const code = typeof error.code === "string" ? error.code : "unknown-error";
    const message =
      typeof error.message === "string"
        ? error.message
        : "KeyNest could not complete the settings request.";
    return new SettingsClientError(code, message);
  }

  return new SettingsClientError(
    "unknown-error",
    "KeyNest could not complete the settings request.",
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export const settingsClient = {
  getSettings: () => invokeSettings<SettingsSnapshot>("get_settings"),
  setAutoLockSeconds: (seconds: AutoLockSeconds) =>
    invokeSettings<SettingsSnapshot>("set_auto_lock_seconds", { seconds }),
  setClipboardClearSeconds: (seconds: ClipboardClearSeconds) =>
    invokeSettings<SettingsSnapshot>("set_clipboard_clear_seconds", { seconds }),
  setTheme: (theme: ThemePreference) =>
    invokeSettings<SettingsSnapshot>("set_theme", { theme }),
  setLaunchAtStartup: (enabled: boolean) =>
    invokeSettings<SettingsSnapshot>("set_launch_at_startup", { enabled }),
  recordActivity: () => invokeSettings<void>("record_activity"),
  openDataFolder: () => invokeSettings<void>("open_keynest_data_folder"),
};
