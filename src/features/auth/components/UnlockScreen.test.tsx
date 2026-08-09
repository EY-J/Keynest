import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { authClient } from "../authClient";
import { AuthClientError } from "../types";
import ResetDialog from "./ResetDialog";
import UnlockScreen from "./UnlockScreen";

vi.mock("../authClient", () => ({
  authClient: {
    unlock: vi.fn(),
  },
}));

describe("UnlockScreen", () => {
  const unlock = vi.mocked(authClient.unlock);

  beforeEach(() => {
    unlock.mockReset();
  });

  it("clears and refocuses the password after invalid credentials", async () => {
    const user = userEvent.setup();
    unlock.mockRejectedValue(
      new AuthClientError(
        "invalid-credentials",
        "The master password is incorrect.",
      ),
    );
    render(<UnlockScreen onUnlocked={vi.fn()} onReset={vi.fn()} />);
    const field = screen.getByLabelText("Master password");

    await user.type(field, "wrong master password");
    await user.keyboard("{Enter}");

    await waitFor(() => expect(field).toHaveValue(""));
    expect(field).toHaveFocus();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "The master password is incorrect.",
    );
  });

  it("unlocks only when the backend returns unlocked", async () => {
    const user = userEvent.setup();
    const onUnlocked = vi.fn();
    unlock.mockResolvedValue("unlocked");
    render(<UnlockScreen onUnlocked={onUnlocked} onReset={vi.fn()} />);

    await user.type(
      screen.getByLabelText("Master password"),
      "a secure master password",
    );
    await user.click(screen.getByRole("button", { name: "Unlock KeyNest" }));

    expect(onUnlocked).toHaveBeenCalledOnce();
    expect(screen.getByLabelText("Master password")).toHaveValue("");
  });
});

describe("ResetDialog", () => {
  it("requires exact RESET before deleting local data", async () => {
    const user = userEvent.setup();
    const onReset = vi.fn().mockResolvedValue(undefined);
    render(<ResetDialog isOpen onClose={vi.fn()} onReset={onReset} />);
    const resetButton = screen.getByRole("button", { name: "Reset KeyNest" });

    expect(resetButton).toBeDisabled();
    await user.type(screen.getByLabelText("Type RESET to confirm"), "reset");
    expect(resetButton).toBeDisabled();
    await user.clear(screen.getByLabelText("Type RESET to confirm"));
    await user.type(screen.getByLabelText("Type RESET to confirm"), "RESET");
    expect(resetButton).toBeEnabled();
    await user.click(resetButton);

    expect(onReset).toHaveBeenCalledWith("RESET");
  });
});
