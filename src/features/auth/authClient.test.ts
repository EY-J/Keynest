import { beforeEach, describe, expect, it, vi } from "vitest";
import { authClient } from "./authClient";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("authClient", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue("unlocked");
  });

  it("uses the exact backend commands and arguments", async () => {
    await authClient.createMasterPassword("a secure master password");
    await authClient.unlock("a secure master password");
    await authClient.lock();
    await authClient.resetKeynest("RESET");

    expect(invokeMock.mock.calls).toEqual([
      ["create_master_password", { password: "a secure master password" }],
      ["unlock", { password: "a secure master password" }],
      ["lock"],
      ["reset_keynest", { confirmation: "RESET" }],
    ]);
  });

  it("normalizes a structured Tauri error", async () => {
    invokeMock.mockRejectedValue({
      code: "throttled",
      message: "Wait a moment before trying again.",
      retryAfterMs: 2_000,
    });

    await expect(authClient.unlock("wrong password")).rejects.toMatchObject({
      name: "AuthClientError",
      code: "throttled",
      message: "Wait a moment before trying again.",
      retryAfterMs: 2_000,
    });
  });
});
