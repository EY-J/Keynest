import { beforeEach, describe, expect, it, vi } from "vitest";
import { settingsClient } from "./settingsClient";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("settingsClient", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({
      autoLockSeconds: 300,
      clipboardClearSeconds: 30,
      theme: "system",
      launchAtStartup: false,
    });
  });

  it("uses the exact backend commands and arguments", async () => {
    await settingsClient.getSettings();
    await settingsClient.setAutoLockSeconds(900);
    await settingsClient.setClipboardClearSeconds(60);
    await settingsClient.setTheme("light");
    await settingsClient.setLaunchAtStartup(true);
    await settingsClient.recordActivity();
    await settingsClient.openDataFolder();

    expect(invokeMock.mock.calls).toEqual([
      ["get_settings"],
      ["set_auto_lock_seconds", { seconds: 900 }],
      ["set_clipboard_clear_seconds", { seconds: 60 }],
      ["set_theme", { theme: "light" }],
      ["set_launch_at_startup", { enabled: true }],
      ["record_activity"],
      ["open_keynest_data_folder"],
    ]);
  });

  it("normalizes structured Tauri errors", async () => {
    invokeMock.mockRejectedValue({
      code: "invalid-settings",
      message: "Saved preferences are unavailable.",
    });

    await expect(settingsClient.getSettings()).rejects.toMatchObject({
      name: "SettingsClientError",
      code: "invalid-settings",
      message: "Saved preferences are unavailable.",
    });
  });
});
