import assert from "node:assert/strict";
import test from "node:test";
import { deferred } from "./componentHarness.mjs";
import { summary, vaultFixture } from "./vaultFixture.mjs";

test("opening/copying a credential never fetches plaintext into React", async () => {
  const f = await vaultFixture();
  assert.deepEqual(f.calls, [["summary", "a"]]);
  assert.equal(f.password().type, "password");
  await f.click("Copy password");
  assert.deepEqual(f.calls, [["summary", "a"], ["copy", "a"]]);
  assert.ok(!JSON.stringify(f.tree()).includes("fixture-secret"));
  f.unmount();
});

test("secure password copy shows fixed-width temporary icon feedback", async () => {
  const f = await vaultFixture();
  await f.click("Copy password");
  const copied = f.find(n => n.type === "button" && n.props["aria-label"] === "Copied");
  assert.equal(copied.props.title, "Copied");
  assert.equal(copied.props.children.type, "Check");
  assert.deepEqual([...f.timers.values()], [1_200]);
  f.expire(); await f.flush();
  const restored = f.find(n => n.type === "button" && n.props["aria-label"] === "Copy password");
  assert.equal(restored.props.children.type, "Copy");
  f.unmount();
});

test("username copy uses the vault client and restores its fixed-width icon", async () => {
  const f = await vaultFixture();
  await f.click("Copy username");
  assert.deepEqual(f.calls.at(-1), ["copy-username", "a"]);
  const copied = f.find(n => n.type === "button" && n.props["aria-label"] === "Copied");
  assert.equal(copied.props.title, "Copied");
  assert.equal(copied.props.children.type, "Check");
  assert.deepEqual([...f.timers.values()], [1_200]);
  f.expire(); await f.flush();
  assert.equal(f.find(n => n.props["aria-label"] === "Copy username").props.children.type, "Copy");
  f.unmount();
});

test("website action uses the external opener and modal favorite reflects canonical props", async () => {
  const f = await vaultFixture();
  const favorite = f.find(n => n.props["aria-label"] === "Add credential to favorites");
  assert.equal(favorite.props["aria-pressed"], false);
  assert.equal(favorite.props.children.type, "Star");
  await f.click("Add credential to favorites");
  assert.deepEqual(f.calls.at(-1), ["favorite", "a"]);

  f.props.isFavorite = true;
  f.render(); await f.flush();
  const active = f.find(n => n.props["aria-label"] === "Remove credential from favorites");
  assert.equal(active.props["aria-pressed"], true);
  assert.equal(active.props.children.props.fill, "currentColor");

  await f.click("Open website");
  assert.deepEqual(f.calls.at(-1), ["open", "https://example.test/"]);
  f.unmount();
});

test("website action rejects unsupported schemes before invoking the opener", async () => {
  const f = await vaultFixture(
    undefined,
    async id => ({ ...summary(id), website: "javascript:alert(1)" }),
  );
  await f.click("Open website");
  assert.equal(f.calls.some(call => call[0] === "open"), false);
  assert.equal(
    f.find(n => n.props.className === "vault-form-error").props.children,
    "KeyNest could not safely open this website.",
  );
  f.unmount();
});

test("credential actions expose stable icon-only names and reveal state", async () => {
  const f = await vaultFixture();
  const reveal = f.find(n => n.type === "button" && n.props["aria-label"] === "Reveal password");
  assert.equal(reveal.props.title, "Reveal password");
  assert.equal(reveal.props.children.type, "Eye");
  assert.equal(f.find(n => n.props["aria-label"] === "Copy password").props.children.type, "Copy");
  assert.equal(f.find(n => n.props["aria-label"] === "Edit credential").props.className,
    "vault-top-action vault-edit-action");
  assert.equal(f.find(n => n.props["aria-label"] === "Delete credential").props.className,
    "vault-top-action vault-delete-action");

  await f.click("Reveal password");
  const hide = f.find(n => n.type === "button" && n.props["aria-label"] === "Hide password");
  assert.equal(hide.props.title, "Hide password");
  assert.equal(hide.props.children.type, "EyeOff");
  f.unmount();
});

test("credential details use identity tags and three structured rows", async () => {
  const f = await vaultFixture();
  assert.equal(f.find(n => n.props.className === "vault-credential-tag").props.children, "work");
  assert.equal(f.nodes().filter(n => n.props.className?.split?.(" ").includes("vault-detail-row")).length, 3);
  assert.equal(f.password().value, "••••••••");
  assert.notEqual(f.password().value, "https://example.test");
  assert.equal(f.find(n => n.props.className === "vault-success"), undefined);
  f.unmount();
});

test("reveal is selected-record-only and hiding releases plaintext state", async () => {
  const f = await vaultFixture();
  await f.click("Reveal password");
  assert.deepEqual(f.calls.at(-1), ["secret", "a"]);
  assert.equal(f.password().value, "fixture-secret-a");
  await f.click("Hide password");
  assert.ok(!JSON.stringify(f.tree()).includes("fixture-secret"));
  await f.click("Reveal password");
  assert.equal(f.calls.filter(c => c[0] === "secret").length, 2, "hidden passwords are not cached");
  await f.click("Close credential");
  assert.equal(f.closed(), true);
  assert.ok(!JSON.stringify(f.tree()).includes("fixture-secret"));
  f.unmount();
});

test("editing fetches only the selected record and cancel discards it", async () => {
  const f = await vaultFixture();
  await f.click("Edit credential");
  const form = f.find(n => n.type === "VaultRecordForm");
  assert.equal(form.props.initialRecord.password, "fixture-secret-a");
  form.props.onCancel(); await f.flush();
  assert.ok(!f.find(n => n.type === "VaultRecordForm"));
  assert.ok(!JSON.stringify(f.tree()).includes("fixture-secret"));
  f.unmount();
});

test("a stale secret response cannot enter a different record dialog", async () => {
  const pending = deferred();
  const f = await vaultFixture(() => pending.promise);
  await f.click("Reveal password");
  f.props.recordId = "b"; f.render(); await f.flush();
  pending.resolve({ ...summary("a"), password: "stale-secret" }); await f.flush();
  assert.equal(f.password().type, "password");
  assert.ok(!JSON.stringify(f.tree()).includes("stale-secret"));
  f.unmount();
});
