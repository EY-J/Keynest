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
    await authClient.changeMasterPassword(
      "current password value",
      "new password value",
    );
    await authClient.resetKeynest("RESET KEYNEST");
    await authClient.resetKeynestAuthenticated(
      "current password value",
      "RESET KEYNEST",
    );

    expect(invokeMock.mock.calls).toEqual([
      ["create_master_password", { password: "a secure master password" }],
      ["unlock", { password: "a secure master password" }],
      ["lock"],
      [
        "change_master_password",
        {
          currentPassword: "current password value",
          newPassword: "new password value",
        },
      ],
      ["reset_keynest", { confirmation: "RESET KEYNEST" }],
      [
        "reset_keynest_authenticated",
        {
          currentPassword: "current password value",
          confirmation: "RESET KEYNEST",
        },
      ],
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
