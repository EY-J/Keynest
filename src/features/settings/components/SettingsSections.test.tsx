import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsProvider from "../SettingsProvider";
import { settingsClient } from "../settingsClient";
import AppearanceSettings from "./AppearanceSettings";
import AboutSettings from "./AboutSettings";
import GeneralSettings from "./GeneralSettings";

const { getVersion } = vi.hoisted(() => ({ getVersion: vi.fn() }));

vi.mock("../settingsClient", () => ({
  settingsClient: {
    getSettings: vi.fn(),
    setTheme: vi.fn(),
    setLaunchAtStartup: vi.fn(),
    openDataFolder: vi.fn(),
  },
}));

vi.mock("@tauri-apps/api/app", () => ({ getVersion }));

const confirmedSettings = {
  autoLockSeconds: 300 as const,
  clipboardClearSeconds: 30 as const,
  theme: "system" as const,
  launchAtStartup: false,
};

function renderWithSettings(children: React.ReactNode) {
  return render(<SettingsProvider>{children}</SettingsProvider>);
}

describe("settings sections", () => {
  const getSettings = vi.mocked(settingsClient.getSettings);
  const setTheme = vi.mocked(settingsClient.setTheme);
  const setLaunchAtStartup = vi.mocked(settingsClient.setLaunchAtStartup);
  const openDataFolder = vi.mocked(settingsClient.openDataFolder);

  beforeEach(() => {
    vi.clearAllMocks();
    getSettings.mockResolvedValue(confirmedSettings);
    setTheme.mockResolvedValue(confirmedSettings);
    setLaunchAtStartup.mockResolvedValue(confirmedSettings);
    openDataFolder.mockResolvedValue(undefined);
    getVersion.mockResolvedValue("0.1.0");
  });

  it("keeps the confirmed launch-at-startup state selected until the backend confirms", async () => {
    const user = userEvent.setup();
    let confirmLaunch!: (value: typeof confirmedSettings) => void;
    setLaunchAtStartup.mockReturnValueOnce(
      new Promise((resolve) => {
        confirmLaunch = resolve;
      }),
    );

    renderWithSettings(<GeneralSettings />);

    const launchAtStartup = await screen.findByRole("checkbox", {
      name: "Launch KeyNest at startup",
    });
    expect(launchAtStartup).not.toBeChecked();
    expect(screen.getByText(/starts minimized and locked/i)).toBeInTheDocument();

    await user.click(launchAtStartup);

    expect(setLaunchAtStartup).toHaveBeenCalledWith(true);
    expect(launchAtStartup).not.toBeChecked();
    expect(launchAtStartup).toBeDisabled();

    await act(async () => {
      confirmLaunch({ ...confirmedSettings, launchAtStartup: true });
    });
    expect(launchAtStartup).toBeChecked();
  });

  it("retains the confirmed launch setting and alerts when its save fails", async () => {
    const user = userEvent.setup();
    setLaunchAtStartup.mockRejectedValueOnce(new Error("backend unavailable"));
    renderWithSettings(<GeneralSettings />);

    const launchAtStartup = await screen.findByRole("checkbox", {
      name: "Launch KeyNest at startup",
    });
    await user.click(launchAtStartup);

    await waitFor(() => {
      expect(launchAtStartup).not.toBeChecked();
      expect(screen.getByRole("alert")).toHaveTextContent(
        "KeyNest could not save this general preference.",
      );
    });
  });

  it("renders only System, Dark, and Light theme choices using the confirmed preference", async () => {
    getSettings.mockResolvedValue({ ...confirmedSettings, theme: "dark" });
    renderWithSettings(<AppearanceSettings />);

    expect(await screen.findByRole("radio", { name: "System" })).not.toBeChecked();
    expect(screen.getByRole("radio", { name: "Dark" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Light" })).not.toBeChecked();
    expect(screen.getAllByRole("radio")).toHaveLength(3);
  });

  it("renders local-only recovery guidance and opens the data folder without IPC arguments", async () => {
    const user = userEvent.setup();
    render(<AboutSettings />);

    expect(document.querySelector(".about-mark")).toBeInTheDocument();
    expect(await screen.findByText("Version 0.1.0")).toBeInTheDocument();
    expect(screen.getByText(
      "Your encrypted KeyNest data stays on this device. KeyNest has no account service and cannot recover a forgotten master password.",
    )).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Open KeyNest data folder" }));
    expect(openDataFolder).toHaveBeenCalledWith();
  });

  it("shows a safe fallback when the app version cannot be read", async () => {
    getVersion.mockRejectedValueOnce(new Error("unavailable"));
    render(<AboutSettings />);

    expect(await screen.findByText("Version unavailable")).toBeInTheDocument();
  });
});
