import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { authClient } from "../authClient";
import type { AuthStatus } from "../types";
import AuthGate from "./AuthGate";

const { listenMock } = vi.hoisted(() => ({
  listenMock: vi.fn(),
}));

vi.mock("../authClient", () => ({
  authClient: {
    getStatus: vi.fn(),
    createMasterPassword: vi.fn(),
    unlock: vi.fn(),
    lock: vi.fn(),
    resetKeynest: vi.fn(),
  },
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

describe("AuthGate", () => {
  const getStatus = vi.mocked(authClient.getStatus);
  const lock = vi.mocked(authClient.lock);

  beforeEach(() => {
    vi.clearAllMocks();
    listenMock.mockResolvedValue(vi.fn());
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

  it("removes protected content when the backend emits the locked event and cleans up its listener", async () => {
    let onLocked!: () => void;
    const unlisten = vi.fn();
    getStatus.mockResolvedValue("unlocked");
    listenMock.mockImplementation(async (eventName, callback) => {
      expect(eventName).toBe("keynest://locked");
      onLocked = callback;
      return unlisten;
    });

    render(<AuthGate>{() => <div>Protected home</div>}</AuthGate>);

    expect(await screen.findByText("Protected home")).toBeInTheDocument();
    act(() => onLocked());

    expect(
      await screen.findByRole("heading", { name: "Welcome back" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("Protected home")).not.toBeInTheDocument();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("fails closed by locking Rust when backend lock-listener registration fails", async () => {
    getStatus.mockResolvedValue("unlocked");
    listenMock.mockRejectedValue(new Error("event bridge unavailable"));
    lock.mockResolvedValue("locked");

    render(<AuthGate>{() => <div>Protected home</div>}</AuthGate>);

    expect(
      await screen.findByRole("heading", { name: "Welcome back" }),
    ).toBeInTheDocument();
    expect(lock).toHaveBeenCalledOnce();
    expect(screen.queryByText("Protected home")).not.toBeInTheDocument();
  });

  it("renders a data error without protected content when fallback lock cannot be confirmed", async () => {
    getStatus.mockResolvedValue("unlocked");
    listenMock.mockRejectedValue(new Error("event bridge unavailable"));
    lock.mockRejectedValue(new Error("IPC unavailable"));

    render(<AuthGate>{() => <div>Protected home</div>}</AuthGate>);

    expect(
      await screen.findByRole("heading", {
        name: "KeyNest could not verify your local data.",
      }),
    ).toBeInTheDocument();
    expect(screen.queryByText("Protected home")).not.toBeInTheDocument();
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
