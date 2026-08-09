import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { authClient } from "../features/auth/authClient";
import App from "./App";

vi.mock("../features/auth/authClient", () => ({
  authClient: {
    getStatus: vi.fn(),
    createMasterPassword: vi.fn(),
    unlock: vi.fn(),
    lock: vi.fn(),
    resetKeynest: vi.fn(),
  },
}));

describe("App master-password integration", () => {
  const getStatus = vi.mocked(authClient.getStatus);
  const lock = vi.mocked(authClient.lock);

  beforeEach(() => {
    vi.clearAllMocks();
    getStatus.mockResolvedValue("unlocked");
    lock.mockResolvedValue("locked");
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
});
