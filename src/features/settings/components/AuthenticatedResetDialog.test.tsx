import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import AuthenticatedResetDialog from "./AuthenticatedResetDialog";

describe("AuthenticatedResetDialog", () => {
  it("requires a current password and the exact RESET KEYNEST phrase", async () => {
    const user = userEvent.setup();
    render(
      <AuthenticatedResetDialog
        isOpen
        onClose={vi.fn()}
        onReset={vi.fn().mockResolvedValue(undefined)}
      />,
    );
    const reset = screen.getByRole("button", { name: "Reset KeyNest" });
    const phrase = screen.getByLabelText("Type RESET KEYNEST to confirm");

    expect(reset).toBeDisabled();
    await user.type(phrase, "RESET KEYNEST");
    expect(reset).toBeDisabled();
    await user.type(screen.getByLabelText("Current master password"), "current password");
    await user.clear(phrase);
    await user.type(phrase, "reset keynest");
    expect(reset).toBeDisabled();
    await user.clear(phrase);
    await user.type(phrase, "RESET");
    expect(reset).toBeDisabled();
    await user.clear(phrase);
    await user.type(phrase, "RESET KEYNEST");
    expect(reset).toBeEnabled();
  });

  it("traps focus and closes after a successful reset while clearing secret fields", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const onReset = vi.fn().mockResolvedValue(undefined);
    render(
      <AuthenticatedResetDialog isOpen onClose={onClose} onReset={onReset} />,
    );
    const currentPassword = screen.getByLabelText("Current master password");
    const phrase = screen.getByLabelText("Type RESET KEYNEST to confirm");

    await user.type(currentPassword, "current password");
    await user.type(phrase, "RESET KEYNEST");
    screen.getByRole("button", { name: "Reset KeyNest" }).focus();
    await user.tab();
    expect(currentPassword).toHaveFocus();

    await user.click(screen.getByRole("button", { name: "Reset KeyNest" }));

    expect(onReset).toHaveBeenCalledWith("current password", "RESET KEYNEST");
    expect(currentPassword).toHaveValue("");
    expect(phrase).toHaveValue("");
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("does not close with Escape while the destructive request is pending", async () => {
    const user = userEvent.setup();
    let resolveReset!: () => void;
    const onClose = vi.fn();
    render(
      <AuthenticatedResetDialog
        isOpen
        onClose={onClose}
        onReset={() =>
          new Promise((resolve) => {
            resolveReset = resolve;
          })
        }
      />,
    );

    await user.type(screen.getByLabelText("Current master password"), "current password");
    await user.type(screen.getByLabelText("Type RESET KEYNEST to confirm"), "RESET KEYNEST");
    await user.click(screen.getByRole("button", { name: "Reset KeyNest" }));
    await user.keyboard("{Escape}");

    expect(onClose).not.toHaveBeenCalled();
    resolveReset();
  });
});
