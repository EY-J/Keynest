import assert from "node:assert/strict";
import test from "node:test";
import { mount, deferred } from "./componentHarness.mjs";

for (const authenticated of [false, true]) {
  test(`${authenticated ? "authenticated" : "locked"} reset retains confirmation through exit and cleans on close`, async () => {
    let closed = 0;
    const f = await mount(authenticated ? "../src/features/settings/components/AuthenticatedResetDialog.tsx" : "../src/features/auth/components/ResetDialog.tsx",
      { isOpen: true, onClose() { closed++; }, onReset: async () => {} },
      { "../../auth/components/PasswordField": { default: "PasswordField" } });
    const modal = () => f.tree().props;
    assert.equal(modal().closeOnBackdrop, undefined, "safe default: no backdrop dismiss");
    f.find(n => n.type === "input").props.onChange({ target: { value: "RESET KEYNEST" } }); await f.flush();
    f.find(n => n.props.children === "Cancel").props.onClick(); await f.flush();
    assert.equal(modal().closing, true); assert.equal(closed, 0);
    assert.equal(f.find(n => n.type === "input").props.value, "RESET KEYNEST");
    modal().onExitComplete(); await f.flush(); assert.equal(closed, 1);
    f.props.isOpen = false; f.render(); await f.flush();
    f.props.isOpen = true; f.render(); await f.flush();
    assert.equal(f.find(n => n.type === "input").props.value, ""); f.unmount();
  });
  test(`${authenticated ? "authenticated" : "locked"} reset preserves pending dismissal guard`, async () => {
    const pending = deferred();
    const f = await mount(authenticated ? "../src/features/settings/components/AuthenticatedResetDialog.tsx" : "../src/features/auth/components/ResetDialog.tsx",
      { isOpen: true, onClose() {}, onReset: () => pending.promise }, { "../../auth/components/PasswordField": { default: "PasswordField" } });
    f.find(n => n.type === "input").props.onChange({ target: { value: "RESET KEYNEST" } });
    if (authenticated) f.find(n => n.type === "PasswordField").props.onChange("fixture password");
    await f.flush();
    f.find(n => n.type === "form").props.onSubmit({ preventDefault() {} }); await f.flush();
    assert.equal(f.tree().props.pending, true);
    assert.equal(f.find(n => n.props.children === "Cancel").props.disabled, true);
    pending.reject(new Error("fixture failure")); await f.flush();
    assert.equal(f.tree().props.closing, false); f.unmount();
  });
}
test("recovery management forbids accidental dismissal of replacement key and waits for acknowledgement exit", async () => {
  const f = await mount("../src/features/settings/components/RecoverySettings.tsx", {}, {
    "lucide-react": { KeyRound: "icon", X: "icon" }, "./SettingsRow": { default: "SettingsRow" },
    "../../auth/components/PasswordField": { default: "PasswordField" },
    "../../auth/components/RecoveryKeyScreen": { RecoveryKeyContent: "RecoveryKeyContent" },
    "../../auth/types": { AuthClientError: Error },
    "../../auth/authClient": { authClient: {
      getRecoveryStatus: async () => ({ configured: true }),
      regenerateRecoveryKey: async () => ({ status: "unlocked", recoveryKey: "fixture-secret-key" }),
      completeRecoveryKeyDisplay: async () => "unlocked",
    } },
  });
  f.find(n => n.props.children === "Manage").props.onClick(); await f.flush();
  f.find(n => n.type === "PasswordField").props.onChange("fixture password"); await f.flush();
  f.find(n => n.type === "form").props.onSubmit({ preventDefault() {} }); await f.flush();
  const modal = () => f.find(n => n.props.titleId === "recovery-key-dialog-title");
  assert.equal(modal().props.closeOnEscape, false);
  assert.equal(modal().props.closeOnBackdrop, undefined);
  await f.find(n => n.type === "RecoveryKeyContent").props.onSaved(); await f.flush();
  assert.equal(modal().props.closing, true);
  assert.ok(f.find(n => n.type === "RecoveryKeyContent"));
  modal().props.onExitComplete(); await f.flush(); assert.ok(!modal());
  assert.doesNotMatch(JSON.stringify(f.tree()), /fixture-secret-key/); f.unmount();
});
