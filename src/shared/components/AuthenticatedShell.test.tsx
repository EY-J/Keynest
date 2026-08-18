import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AuthenticatedShell from "./AuthenticatedShell";

const useSettings = vi.fn();

vi.mock("../../features/settings/SettingsProvider", () => ({
  useSettings: () => useSettings(),
}));

describe("AuthenticatedShell", () => {
  beforeEach(() => {
    useSettings.mockReturnValue({
      settings: {
        autoLockSeconds: 300,
        clipboardClearSeconds: 30,
        theme: "system",
        launchAtStartup: false,
      },
    });
  });

  it("navigates between Home and Settings and marks the active destination", async () => {
    const user = userEvent.setup();
    render(
      <AuthenticatedShell
        onLockKeynest={vi.fn().mockResolvedValue(undefined)}
        onOpenPasswordVault={vi.fn()}
        onResetAuthenticated={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(
      screen.getByRole("heading", { name: /secure nest/i }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Open navigation" }));
    await user.click(screen.getByRole("button", { name: "Settings" }));

    expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Settings", hidden: true }),
    ).toHaveClass("active");
    expect(screen.getByRole("complementary", { hidden: true })).toHaveAttribute(
      "aria-hidden",
      "true",
    );

    await user.click(screen.getByRole("button", { name: "Open navigation" }));
    await user.click(screen.getByRole("button", { name: "Home" }));

    expect(
      screen.getByRole("heading", { name: /secure nest/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Home", hidden: true }),
    ).toHaveClass("active");
  });

  it("closes the navigation with Escape and its backdrop", async () => {
    const user = userEvent.setup();
    render(
      <AuthenticatedShell
        onLockKeynest={vi.fn().mockResolvedValue(undefined)}
        onOpenPasswordVault={vi.fn()}
        onResetAuthenticated={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Open navigation" }));
    await user.keyboard("{Escape}");
    expect(screen.getByRole("complementary", { hidden: true })).toHaveAttribute(
      "aria-hidden",
      "true",
    );

    await user.click(screen.getByRole("button", { name: "Open navigation" }));
    await user.click(screen.getByRole("button", { name: "Close navigation" }));
    expect(screen.getByRole("complementary", { hidden: true })).toHaveAttribute(
      "aria-hidden",
      "true",
    );
  });

  it("keeps manual lock and Password Vault callbacks available from the shell", async () => {
    const user = userEvent.setup();
    const onLockKeynest = vi.fn().mockResolvedValue(undefined);
    const onOpenPasswordVault = vi.fn();
    render(
      <AuthenticatedShell
        onLockKeynest={onLockKeynest}
        onOpenPasswordVault={onOpenPasswordVault}
        onResetAuthenticated={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Open navigation" }));
    await user.click(screen.getByRole("button", { name: "Password Vault" }));
    expect(onOpenPasswordVault).toHaveBeenCalledOnce();
    expect(screen.getByRole("complementary", { hidden: true })).toHaveAttribute(
      "aria-hidden",
      "true",
    );

    await user.click(screen.getByRole("button", { name: "Open navigation" }));
    await user.click(screen.getByRole("button", { name: "Lock KeyNest" }));
    expect(onLockKeynest).toHaveBeenCalledOnce();
  });

  it("shows a safe non-blocking warning banner when settings report a warning", () => {
    useSettings.mockReturnValue({
      settings: {
        autoLockSeconds: 300,
        clipboardClearSeconds: 30,
        theme: "system",
        launchAtStartup: false,
        warning: "KeyNest repaired saved preferences using secure defaults.",
      },
    });

    render(
      <AuthenticatedShell
        onLockKeynest={vi.fn().mockResolvedValue(undefined)}
        onOpenPasswordVault={vi.fn()}
        onResetAuthenticated={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(screen.getByRole("status")).toHaveTextContent(
      "KeyNest repaired saved preferences using secure defaults.",
    );
    expect(
      screen.getByRole("heading", { name: /secure nest/i }),
    ).toBeInTheDocument();
  });
});
