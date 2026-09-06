import assert from "node:assert/strict";
import test from "node:test";
import { mount } from "./componentHarness.mjs";

for (const initial of ["unlocked", "locked", "setup-required"]) {
  test(`lock event clears the ${initial} screen, including one-time recovery display`, async () => {
    let status = initial;
    const listeners = new Set();
    const f = await mount("../src/features/auth/components/AuthGate.tsx", {
      children: () => ({ type: "sensitive-content", props: {} }),
    }, {
      "@tauri-apps/api/event": { listen: async (_name, fn) => { listeners.add(fn); return () => listeners.delete(fn); } },
      "../authClient": { authClient: { getStatus: async () => status, lock: async () => "locked" } },
      "../types": { AuthClientError: Error },
      ...Object.fromEntries(["AuthLayout", "DataErrorScreen", "SetupScreen", "UnlockScreen"].map(name => [`./${name}`, { default: name }])),
    });
    assert.ok(listeners.size > 0);
    const oldKey = f.tree().key;
    status = "locked";
    listeners.forEach(fn => fn()); await f.flush();
    assert.equal(f.tree().type, "UnlockScreen");
    assert.notEqual(f.tree().key, oldKey);
    assert.ok(!f.find(n => n.type === "sensitive-content"));
    const key = f.tree().key;
    listeners.forEach(fn => fn()); await f.flush();
    assert.notEqual(f.tree().key, key, "same-status lock remounts recovery-bearing unlock screen");
    f.unmount(); assert.equal(listeners.size, 0);
  });
}
