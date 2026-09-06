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

test("reveal is selected-record-only and hiding releases plaintext state", async () => {
  const f = await vaultFixture();
  await f.click("Reveal");
  assert.deepEqual(f.calls.at(-1), ["secret", "a"]);
  assert.equal(f.password().value, "fixture-secret-a");
  await f.click("Hide");
  assert.ok(!JSON.stringify(f.tree()).includes("fixture-secret"));
  await f.click("Reveal");
  assert.equal(f.calls.filter(c => c[0] === "secret").length, 2, "hidden passwords are not cached");
  await f.click("Close credential");
  assert.equal(f.closed(), true);
  assert.ok(!JSON.stringify(f.tree()).includes("fixture-secret"));
  f.unmount();
});

test("editing fetches only the selected record and cancel discards it", async () => {
  const f = await vaultFixture();
  await f.click("Edit");
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
  await f.click("Reveal");
  f.props.recordId = "b"; f.render(); await f.flush();
  pending.resolve({ ...summary("a"), password: "stale-secret" }); await f.flush();
  assert.equal(f.password().type, "password");
  assert.ok(!JSON.stringify(f.tree()).includes("stale-secret"));
  f.unmount();
});
