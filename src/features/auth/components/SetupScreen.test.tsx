import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { authClient } from "../authClient";
import SetupScreen from "./SetupScreen";

vi.mock("../authClient", () => ({
  authClient: {
    createMasterPassword: vi.fn(),
  },
}));

describe("SetupScreen", () => {
  const createMasterPassword = vi.mocked(authClient.createMasterPassword);

  beforeEach(() => {
    createMasterPassword.mockReset();
  });

  it("rejects a master password shorter than twelve characters", async () => {
    const user = userEvent.setup();
    render(<SetupScreen onCreated={vi.fn()} />);

    await user.type(screen.getByLabelText("Master password"), "short");
    await user.type(screen.getByLabelText("Confirm master password"), "short");
    await user.click(
      screen.getByRole("button", { name: "Create Master Password" }),
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Use at least 12 characters.",
    );
    expect(createMasterPassword).not.toHaveBeenCalled();
  });

  it("rejects a confirmation that does not match", async () => {
    const user = userEvent.setup();
    render(<SetupScreen onCreated={vi.fn()} />);

    await user.type(
      screen.getByLabelText("Master password"),
      "a secure master password",
    );
    await user.type(
      screen.getByLabelText("Confirm master password"),
      "a different master password",
    );
    await user.click(
      screen.getByRole("button", { name: "Create Master Password" }),
    );

    expect(screen.getByText("The passwords do not match.")).toBeInTheDocument();
    expect(createMasterPassword).not.toHaveBeenCalled();
  });

  it("clears both fields after successful creation", async () => {
    const user = userEvent.setup();
    const onCreated = vi.fn();
    createMasterPassword.mockResolvedValue("unlocked");
    render(<SetupScreen onCreated={onCreated} />);
    const password = screen.getByLabelText("Master password");
    const confirmation = screen.getByLabelText("Confirm master password");

    await user.type(password, "a secure master password");
    await user.type(confirmation, "a secure master password");
    await user.click(
      screen.getByRole("button", { name: "Create Master Password" }),
    );

    expect(onCreated).toHaveBeenCalledOnce();
    expect(password).toHaveValue("");
    expect(confirmation).toHaveValue("");
  });
});
