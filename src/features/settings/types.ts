export type AutoLockSeconds = 60 | 300 | 900 | 1800;
export type ClipboardClearSeconds = 10 | 30 | 60;
export type ThemePreference = "system" | "dark" | "light";

export type SettingsSnapshot = {
  autoLockSeconds: AutoLockSeconds;
  clipboardClearSeconds: ClipboardClearSeconds;
  theme: ThemePreference;
  launchAtStartup: boolean;
  warning?: string;
};

export class SettingsClientError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "SettingsClientError";
    this.code = code;
  }
}
