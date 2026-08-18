import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsProvider from "../SettingsProvider";
import { settingsClient } from "../settingsClient";
import SecuritySettings from "./SecuritySettings";

vi.mock("../settingsClient", () => ({
  settingsClient: {
    getSettings: vi.fn(),
    setAutoLockSeconds: vi.fn(),
    setClipboardClearSeconds: vi.fn(),
  },
}));

function renderSecuritySettings() {
  return render(
    <SettingsProvider>
      <SecuritySettings />
    </SettingsProvider>,
  );
}

describe("SecuritySettings", () => {
  const getSettings = vi.mocked(settingsClient.getSettings);
  const setAutoLockSeconds = vi.mocked(settingsClient.setAutoLockSeconds);
  const setClipboardClearSeconds = vi.mocked(
    settingsClient.setClipboardClearSeconds,
  );

  beforeEach(() => {
    vi.clearAllMocks();
    getSettings.mockResolvedValue({
      autoLockSeconds: 300,
      clipboardClearSeconds: 30,
      theme: "system",
      launchAtStartup: false,
    });
    setAutoLockSeconds.mockResolvedValue({
      autoLockSeconds: 900,
      clipboardClearSeconds: 30,
      theme: "system",
      launchAtStartup: false,
    });
    setClipboardClearSeconds.mockResolvedValue({
      autoLockSeconds: 300,
      clipboardClearSeconds: 60,
      theme: "system",
      launchAtStartup: false,
    });
  });

  it("offers only the enforced auto-lock and clipboard timeout options", async () => {
    renderSecuritySettings();

    expect(await screen.findByRole("option", { name: "1 minute" })).toHaveValue("60");
    expect(screen.getByRole("option", { name: "5 minutes" })).toHaveValue("300");
    expect(screen.getByRole("option", { name: "15 minutes" })).toHaveValue("900");
    expect(screen.getByRole("option", { name: "30 minutes" })).toHaveValue("1800");
    expect(screen.queryByRole("option", { name: /never/i })).not.toBeInTheDocument();

    const clipboardClear = screen.getByLabelText("Clear clipboard after");
    expect(clipboardClear).toHaveTextContent("10 seconds");
    expect(clipboardClear).toHaveTextContent("30 seconds");
    expect(clipboardClear).toHaveTextContent("60 seconds");
    expect(clipboardClear.querySelectorAll("option")).toHaveLength(3);
    expect(screen.getByText("Lock when Windows sleeps").parentElement).toHaveTextContent(
      "Enabled",
    );
  });

  it("keeps the last confirmed timeout selected while a mutation is pending or rejected", async () => {
    const user = userEvent.setup();
    let resolveMutation!: (value: {
      autoLockSeconds: 900;
      clipboardClearSeconds: 30;
      theme: "system";
      launchAtStartup: false;
    }) => void;
    setAutoLockSeconds.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveMutation = resolve;
      }),
    );
    renderSecuritySettings();

    const autoLock = await screen.findByLabelText("Lock KeyNest after inactivity");
    await user.selectOptions(autoLock, "900");

    expect(autoLock).toHaveValue("300");
    expect(autoLock).toBeDisabled();

    await act(async () => {
      resolveMutation({
        autoLockSeconds: 900,
        clipboardClearSeconds: 30,
        theme: "system",
        launchAtStartup: false,
      });
    });
    expect(autoLock).toHaveValue("900");

    setClipboardClearSeconds.mockRejectedValueOnce(new Error("backend detail"));
    const clipboardClear = screen.getByLabelText("Clear clipboard after");
    await user.selectOptions(clipboardClear, "60");

    await waitFor(() => {
      expect(clipboardClear).toHaveValue("30");
      expect(screen.getByRole("alert")).toHaveTextContent(
        "KeyNest could not save this security preference.",
      );
    });
  });
});
