import assert from "node:assert/strict";
import test from "node:test";
import { mount, deferred } from "./componentHarness.mjs";
async function fixture(initial = "unlocked", lock = async () => "locked") {
  let calls = 0;
  const f = await mount("../src/features/auth/components/AuthGate.tsx", {
    children: () => ({ type: "sensitive-content", props: {} }),
  }, {
    "@tauri-apps/api/event": { listen: async () => () => {} },
    "../authClient": { authClient: { getStatus: async () => initial, lock: () => { calls++; return lock(); } } },
    "../types": { AuthClientError: Error },
    ...Object.fromEntries(["AuthLayout", "DataErrorScreen", "SetupScreen", "UnlockScreen"].map(name => [`./${name}`, { default: name }])),
  });
  return { ...f, calls: () => calls, key(overrides = {}) {
    let prevented = false;
    const event = { key: "L", ctrlKey: true, shiftKey: true, altKey: false, metaKey: false,
      repeat: false, isComposing: false, preventDefault() { prevented = true; }, stopPropagation() {}, ...overrides };
    f.listeners.get("keydown")?.forEach(fn => fn(event));
    return prevented;
  } };
}
test("shortcut calls secure lock, tears down sensitive content, and is safe when locked", async () => {
  const f = await fixture();
  assert.equal(f.key(), true); await f.flush();
  assert.equal(f.calls(), 1); assert.equal(f.tree().type, "UnlockScreen");
  assert.ok(!f.find(n => n.type === "sensitive-content"));
  f.key(); assert.equal(f.calls(), 1); f.unmount();
  assert.equal(f.listeners.get("keydown").size, 0);
});
test("pending/repeated/composing/unrelated keys do not duplicate locking; failure is visible", async () => {
  const pending = deferred(), f = await fixture("unlocked", () => pending.promise);
  for (const overrides of [{ ctrlKey: false }, { shiftKey: false }, { altKey: true }, { metaKey: true }, { isComposing: true }, { key: "x" }, { repeat: true }]) f.key(overrides);
  assert.equal(f.calls(), 0);
  f.key(); f.key(); assert.equal(f.calls(), 1);
  pending.reject(new Error("fixture")); await f.flush();
  assert.ok(f.find(n => n.props.role === "alert"));
  assert.ok(f.find(n => n.type === "sensitive-content"), "failed lock must not fake success");
  f.unmount();
});
