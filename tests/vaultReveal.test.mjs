import assert from "node:assert/strict";
import test from "node:test";
import { deferred, mount } from "./componentHarness.mjs";
import { summary, vaultFixture } from "./vaultFixture.mjs";

test("credential reveal auto-hides after 12 seconds and refetches on next reveal", async () => {
  const f = await vaultFixture();
  await f.click("Reveal password");
  assert.deepEqual([...f.timers.values()], [12_000]);
  f.expire(); await f.flush();
  assert.equal(f.password().type, "password");
  assert.ok(!JSON.stringify(f.tree()).includes("fixture-secret"));
  assert.equal(f.timers.size, 0);
  await f.click("Reveal password");
  assert.equal(f.calls.filter(c => c[0] === "secret").length, 2);
  f.unmount(); assert.equal(f.timers.size, 0);
});

for (const event of ["blur", "visibilitychange"]) {
  test(`${event} masks visible passwords and invalidates a pending reveal`, async () => {
    const f = await vaultFixture();
    await f.click("Reveal password");
    if (event === "visibilitychange") f.document.hidden = true;
    f.fire(event); await f.flush();
    assert.equal(f.password().type, "password");
    assert.equal(f.timers.size, 0);
    f.unmount();

    const pending = deferred();
    const p = await vaultFixture(() => pending.promise);
    await p.click("Reveal password");
    if (event === "visibilitychange") p.document.hidden = true;
    p.fire(event);
    pending.resolve({ ...summary("a"), password: "late-secret" }); await p.flush();
    assert.equal(p.password().type, "password");
    assert.ok(!JSON.stringify(p.tree()).includes("late-secret"));
    p.unmount();
  });
}

test("close/reopen, credential switch and lock/navigation unmount discard reveal timers", async () => {
  const f = await vaultFixture();
  await f.click("Reveal password");
  await f.click("Close credential");
  assert.equal(f.timers.size, 0); f.unmount();
  const reopened = await vaultFixture();
  assert.equal(reopened.password().type, "password");
  await reopened.click("Reveal password");
  reopened.props.credentialId = "b"; reopened.render(); await reopened.flush();
  assert.equal(reopened.password().type, "password");
  assert.equal(reopened.timers.size, 0);
  await reopened.click("Reveal password");
  // AuthGate's lock test verifies that lock removes the authenticated subtree.
  reopened.unmount();
  assert.equal(reopened.timers.size, 0);
  assert.ok([...reopened.listeners.values()].every(set => set.size === 0));
});

test("editor auto-masks and cleans its timer while preserving editable password", async () => {
  const f = await mount("../src/features/vault/components/CredentialForm.tsx", {
    initialRecord: { ...summary("a"), password: "editable-secret" },
    onSubmit: async () => {}, onCancel() {},
  }, {
    "../passwordGenerator": { generateAdvancedPassword: () => "generated-fixture" },
    "../../../components/ui/PasswordStrengthMeter": { default: "PasswordStrengthMeter" },
  });
  const toggle = () => f.find(n => n.props["aria-label"] === "Show password").props.onClick();
  const password = () => f.find(n => n.props.id === "vault-password").props;
  toggle(); await f.flush();
  assert.equal(password().type, "text");
  assert.deepEqual([...f.timers.values()], [12_000]);
  f.expire(); await f.flush();
  assert.equal(password().type, "password");
  assert.equal(password().value, "editable-secret");
  toggle(); await f.flush();
  f.fire("blur"); await f.flush();
  assert.equal(password().type, "password");
  assert.equal(f.timers.size, 0);
  toggle(); await f.flush(); f.unmount();
  assert.equal(f.timers.size, 0);
});
