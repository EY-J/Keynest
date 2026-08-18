import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { authClient } from "../features/auth/authClient";
import { settingsClient } from "../features/settings/settingsClient";
import App from "./App";

const { listenMock } = vi.hoisted(() => ({
  listenMock: vi.fn(),
}));

vi.mock("../features/auth/authClient", () => ({
  authClient: {
    getStatus: vi.fn(),
    createMasterPassword: vi.fn(),
    unlock: vi.fn(),
    lock: vi.fn(),
    resetKeynest: vi.fn(),
  },
}));

vi.mock("../features/settings/settingsClient", () => ({
  settingsClient: {
    getSettings: vi.fn(),
    setAutoLockSeconds: vi.fn(),
    setClipboardClearSeconds: vi.fn(),
    setTheme: vi.fn(),
    setLaunchAtStartup: vi.fn(),
    recordActivity: vi.fn(),
    openDataFolder: vi.fn(),
  },
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

describe("App master-password integration", () => {
  const getStatus = vi.mocked(authClient.getStatus);
  const lock = vi.mocked(authClient.lock);
  const getSettings = vi.mocked(settingsClient.getSettings);

  beforeEach(() => {
    vi.clearAllMocks();
    listenMock.mockResolvedValue(vi.fn());
    vi.mocked(settingsClient.recordActivity).mockResolvedValue(undefined);
    getStatus.mockResolvedValue("unlocked");
    lock.mockResolvedValue("locked");
    getSettings.mockResolvedValue({
      autoLockSeconds: 300,
      clipboardClearSeconds: 30,
      theme: "system",
      launchAtStartup: false,
    });
  });

  it("loads preferences before it renders the authentication flow", async () => {
    let resolveSettings!: (value: {
      autoLockSeconds: 300;
      clipboardClearSeconds: 30;
      theme: "system";
      launchAtStartup: false;
    }) => void;
    getSettings.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveSettings = resolve;
      }),
    );

    render(<App />);

    expect(
      screen.getByRole("heading", { name: "Preparing your nest…" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("Securing your nest…")).not.toBeInTheDocument();

    resolveSettings({
      autoLockSeconds: 300,
      clipboardClearSeconds: 30,
      theme: "system",
      launchAtStartup: false,
    });

    expect(
      await screen.findByText(/Keep your important information inside your/),
    ).toBeInTheDocument();
  });

  it("uses the sidebar action to lock Rust and remove protected content", async () => {
    const user = userEvent.setup();
    render(<App />);
    expect(
      await screen.findByText(/Keep your important information inside your/),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Open navigation" }));
    await user.click(screen.getByRole("button", { name: "Lock KeyNest" }));

    expect(lock).toHaveBeenCalledOnce();
    expect(
      await screen.findByRole("heading", { name: "Welcome back" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/Keep your important information inside your/),
    ).not.toBeInTheDocument();
  });

  it("moves between the authenticated Home and Settings destinations", async () => {
    const user = userEvent.setup();
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: /secure nest/i }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Open navigation" }));
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Open navigation" }));
    await user.click(screen.getByRole("button", { name: "Home" }));
    expect(
      screen.getByRole("heading", { name: /secure nest/i }),
    ).toBeInTheDocument();
  });
});
