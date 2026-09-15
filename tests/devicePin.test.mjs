import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { mount } from "./componentHarness.mjs";

class AuthClientError extends Error {
  constructor(code, message, retryAfterMs) {
    super(message);
    this.code = code;
    this.retryAfterMs = retryAfterMs;
  }
}

function dependencies(authClient) {
  return {
    "../authClient": { authClient },
    "../types": { AuthClientError },
    "./AuthLayout": { default: "AuthLayout" },
    "./PasswordField": { default: "PasswordField" },
    "./UnauthenticatedResetDialog": { default: "UnauthenticatedResetDialog" },
    "./RecoverPasswordDialog": { default: "RecoverPasswordDialog" },
    "./RecoveryKeyScreen": { default: "RecoveryKeyScreen" },
    "./LockScreenBackground": { default: "LockScreenBackground" },
  };
}

test("configured PIN is offered first, accepts only six digits, and unlocks through PIN IPC", async () => {
  const calls = [];
  const authClient = {
    getPinStatus: async () => ({ configured: true, unlockAvailable: true }),
    unlockWithPin: async pin => { calls.push(["pin", pin]); return "unlocked"; },
    unlock: async password => { calls.push(["master", password]); return "unlocked"; },
  };
  let unlocked = 0;
  const view = await mount(
    "../src/features/auth/components/UnlockScreen.tsx",
    { onUnlocked: () => { unlocked++; }, onReset: async () => {} },
    dependencies(authClient),
  );
  await view.flush();
  const layout = view.find(node => node.type === "AuthLayout");
  assert.equal(layout.props.eyebrow, "KeyNest");
  assert.equal(layout.props.title, "Welcome back");
  assert.equal(layout.props.description, undefined);
  assert.match(view.find(node => node.type === "form").props.className, /\bauth-pin-form\b/);
  const pinModeLabel = view.find(node => /\bauth-mode-label--pin\b/.test(node.props?.className ?? ""));
  assert.equal(pinModeLabel.props.children, "Enter PIN");
  const label = view.find(node => node.props?.htmlFor === "device-pin-unlock");
  assert.equal(label.props.className, "sr-only");
  assert.equal(label.props.children, "Device PIN");
  assert.equal(
    view.find(node => /\bauth-unlock-button\b/.test(node.props?.className ?? "")).props.children,
    "Unlock",
  );
  const input = view.find(node => node.props?.id === "device-pin-unlock");
  assert.equal(input.props.inputMode, "numeric");
  assert.equal(input.props.maxLength, 6);
  input.props.onChange({ target: { value: "12a34567" } });
  await view.flush();
  assert.equal(view.find(node => node.props?.id === "device-pin-unlock").props.value, "123456");
  view.find(node => node.type === "form").props.onSubmit({ preventDefault() {} });
  await view.flush();
  assert.deepEqual(calls, [["pin", "123456"]]);
  assert.equal(unlocked, 1);
});

test("PIN lockout switches to Master Password and leaves that fallback usable", async () => {
  const calls = [];
  const authClient = {
    getPinStatus: async () => ({ configured: true, unlockAvailable: true }),
    unlockWithPin: async pin => {
      calls.push(["pin", pin]);
      throw new AuthClientError(
        "pin-requires-master-password",
        "Too many incorrect PIN attempts. Use your Master Password to unlock KeyNest.",
      );
    },
    unlock: async password => { calls.push(["master", password]); return "unlocked"; },
  };
  const view = await mount(
    "../src/features/auth/components/UnlockScreen.tsx",
    { onUnlocked() {}, onReset: async () => {} },
    dependencies(authClient),
  );
  await view.flush();
  view.find(node => node.props?.id === "device-pin-unlock").props.onChange({ target: { value: "000000" } });
  await view.flush();
  view.find(node => node.type === "form").props.onSubmit({ preventDefault() {} });
  await view.flush();
  const masterLayout = view.find(node => node.type === "AuthLayout");
  assert.equal(masterLayout.props.eyebrow, "KeyNest");
  assert.equal(masterLayout.props.title, "Welcome back");
  assert.equal(masterLayout.props.description, undefined);
  assert.match(view.find(node => node.type === "form").props.className, /\bauth-master-form\b/);
  const masterModeLabel = view.find(node => /\bauth-mode-label--password\b/.test(node.props?.className ?? ""));
  assert.equal(masterModeLabel.props.children, "Enter Master Password");
  assert.equal(view.find(node => node.type === "PasswordField").props.visuallyHideLabel, true);
  assert.equal(
    view.find(node => /\bauth-unlock-button\b/.test(node.props?.className ?? "")).props.children,
    "Unlock",
  );
  assert.match(view.find(node => node.props?.role === "alert").props.children, /Too many incorrect PIN attempts/);
  const master = view.find(node => node.type === "PasswordField");
  master.props.onChange("a secure master password");
  await view.flush();
  view.find(node => node.type === "form").props.onSubmit({ preventDefault() {} });
  await view.flush();
  assert.deepEqual(calls, [["pin", "000000"], ["master", "a secure master password"]]);
});

test("settings disclose PIN assurance and expose setup, change, and removal flows", async () => {
  const source = await readFile(
    new URL("../src/features/settings/components/DevicePinSettings.tsx", import.meta.url),
    "utf8",
  );
  assert.match(source, /lower-assurance convenience unlock/);
  assert.match(source, /Current Master Password/);
  for (const action of ["setupPin", "changePin", "removePin", "Change PIN", "Remove PIN"]) {
    assert.match(source, new RegExp(action.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
});
