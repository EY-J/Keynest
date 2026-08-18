import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { authClient } from "../../auth/authClient";
import { AuthClientError } from "../../auth/types";
import ChangeMasterPasswordForm from "./ChangeMasterPasswordForm";

vi.mock("../../auth/authClient", () => ({
  authClient: {
    changeMasterPassword: vi.fn(),
    lock: vi.fn(),
  },
}));

describe("ChangeMasterPasswordForm", () => {
  const changeMasterPassword = vi.mocked(authClient.changeMasterPassword);
  const lock = vi.mocked(authClient.lock);

  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("rejects empty current passwords, fewer than twelve Unicode characters, and mismatched new passwords", async () => {
    const user = userEvent.setup();
    render(<ChangeMasterPasswordForm />);

    await user.click(screen.getByRole("button", { name: "Change master password" }));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Enter your current master password.",
    );

    await user.type(screen.getByLabelText("Current master password"), "current password");
    await user.type(screen.getByLabelText("New master password"), "😀😀😀😀😀😀😀😀😀😀😀");
    await user.type(screen.getByLabelText("Confirm new master password"), "😀😀😀😀😀😀😀😀😀😀😀");
    await user.click(screen.getByRole("button", { name: "Change master password" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Use at least 12 characters.");

    await user.clear(screen.getByLabelText("New master password"));
    await user.clear(screen.getByLabelText("Confirm new master password"));
    await user.type(screen.getByLabelText("New master password"), "a replacement password");
    await user.type(screen.getByLabelText("Confirm new master password"), "different replacement password");
    await user.click(screen.getByRole("button", { name: "Change master password" }));
    expect(screen.getByRole("alert")).toHaveTextContent("The passwords do not match.");
    expect(changeMasterPassword).not.toHaveBeenCalled();
  });

  it("disables every password control while the change is pending", async () => {
    const user = userEvent.setup();
    let resolveChange!: (status: "unlocked") => void;
    changeMasterPassword.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveChange = resolve;
      }),
    );
    render(<ChangeMasterPasswordForm />);

    await fillValidPasswords(user);
    await user.click(screen.getByRole("button", { name: "Change master password" }));

    expect(screen.getByLabelText("Current master password")).toBeDisabled();
    expect(screen.getByLabelText("New master password")).toBeDisabled();
    expect(screen.getByLabelText("Confirm new master password")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Changingâ€¦" })).toBeDisabled();

    resolveChange("unlocked");
    await screen.findByText(
      "Master password changed. Your new password will be required the next time KeyNest locks.",
    );
  });

  it("shows the safe bad-current-password message and clears submitted fields", async () => {
    const user = userEvent.setup();
    changeMasterPassword.mockRejectedValueOnce(
      new AuthClientError("invalid-credentials", "The master password is incorrect."),
    );
    render(<ChangeMasterPasswordForm />);

    await fillValidPasswords(user);
    await user.click(screen.getByRole("button", { name: "Change master password" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "The master password is incorrect.",
    );
    expect(screen.getByLabelText("Current master password")).toHaveValue("");
    expect(screen.getByLabelText("New master password")).toHaveValue("");
    expect(screen.getByLabelText("Confirm new master password")).toHaveValue("");
  });

  it("keeps KeyNest unlocked while showing success and clearing the submitted passwords", async () => {
    const user = userEvent.setup();
    changeMasterPassword.mockResolvedValueOnce("unlocked");
    render(<ChangeMasterPasswordForm />);

    await fillValidPasswords(user);
    await user.click(screen.getByRole("button", { name: "Change master password" }));

    expect(changeMasterPassword).toHaveBeenCalledWith(
      "current password value",
      "a new secure password",
    );
    expect(
      await screen.findByText(
        "Master password changed. Your new password will be required the next time KeyNest locks.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Current master password")).toHaveValue("");
    expect(screen.getByLabelText("New master password")).toHaveValue("");
    expect(screen.getByLabelText("Confirm new master password")).toHaveValue("");
    expect(lock).not.toHaveBeenCalled();
  });
});

async function fillValidPasswords(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText("Current master password"), "current password value");
  await user.type(screen.getByLabelText("New master password"), "a new secure password");
  await user.type(screen.getByLabelText("Confirm new master password"), "a new secure password");
}
