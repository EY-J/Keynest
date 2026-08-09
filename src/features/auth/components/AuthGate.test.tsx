import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { authClient } from "../authClient";
import type { AuthStatus } from "../types";
import AuthGate from "./AuthGate";

vi.mock("../authClient", () => ({
  authClient: {
    getStatus: vi.fn(),
    createMasterPassword: vi.fn(),
    unlock: vi.fn(),
    lock: vi.fn(),
    resetKeynest: vi.fn(),
  },
}));

describe("AuthGate", () => {
  const getStatus = vi.mocked(authClient.getStatus);
  const lock = vi.mocked(authClient.lock);

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("does not mount protected content until Rust reports unlocked", async () => {
    let resolveStatus!: (status: AuthStatus) => void;
    getStatus.mockReturnValue(
      new Promise<AuthStatus>((resolve) => {
        resolveStatus = resolve;
      }),
    );
    render(<AuthGate>{() => <div>Protected home</div>}</AuthGate>);

    expect(screen.queryByText("Protected home")).not.toBeInTheDocument();
    expect(screen.getByText("Securing your nest…")).toBeInTheDocument();

    resolveStatus("locked");
    expect(
      await screen.findByRole("heading", { name: "Welcome back" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("Protected home")).not.toBeInTheDocument();
  });

  it("renders protected content only for the unlocked status", async () => {
    getStatus.mockResolvedValue("unlocked");

    render(<AuthGate>{() => <div>Protected home</div>}</AuthGate>);

    expect(await screen.findByText("Protected home")).toBeInTheDocument();
  });

  it("fails closed when status lookup rejects", async () => {
    getStatus.mockRejectedValue(new Error("IPC unavailable"));

    render(<AuthGate>{() => <div>Protected home</div>}</AuthGate>);

    expect(
      await screen.findByText("KeyNest could not verify your local data."),
    ).toBeInTheDocument();
    expect(screen.queryByText("Protected home")).not.toBeInTheDocument();
  });

  it("unmounts protected content after Rust confirms manual lock", async () => {
    const user = userEvent.setup();
    getStatus.mockResolvedValue("unlocked");
    lock.mockResolvedValue("locked");
    render(
      <AuthGate>
        {({ lock: lockKeynest }) => (
          <button onClick={() => void lockKeynest()}>Protected lock</button>
        )}
      </AuthGate>,
    );
    const lockButton = await screen.findByRole("button", {
      name: "Protected lock",
    });

    await user.click(lockButton);

    expect(
      await screen.findByRole("heading", { name: "Welcome back" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Protected lock" })).not.toBeInTheDocument();
  });
});
