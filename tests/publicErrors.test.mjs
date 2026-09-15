import assert from "node:assert/strict";
import test from "node:test";
import { readFile } from "node:fs/promises";
import vm from "node:vm";
import { transformWithOxc } from "vite";
const source = await readFile(new URL("../src/shared/security/publicErrors.ts", import.meta.url), "utf8");
const { code } = await transformWithOxc(source, "publicErrors.ts");
const module = new vm.SourceTextModule(code);
await module.link(() => {}); await module.evaluate();
const { publicError } = module.namespace;
test("all Rust public codes are handled without exposing transport details", async () => {
  const rust = await readFile(new URL("../src-tauri/src/ipc.rs", import.meta.url), "utf8");
  const codes = [...rust.matchAll(/(?:Self|PublicIpcError)::new\(\s*"([^"]+)"/g)].map(m => m[1]);
  for (const code of [...codes, "throttled", "pin-throttled"]) {
    const result = publicError({ code, message: "sentinel-secret C:\\private\\profile.json SQL nonce stack" });
    assert.equal(result.code, code);
    assert.doesNotMatch(result.message, /sentinel|SQL|nonce|stack|C:\\/);
  }
});
test("unknown/malformed errors are generic and cooldown is bounded", () => {
  for (const error of [null, "sentinel", new Error("sentinel"), { code: "sentinel", message: "sentinel" }, { code: "__proto__" }]) {
    assert.equal(publicError(error).code, "unknown-error");
    assert.doesNotMatch(publicError(error).message, /sentinel/);
  }
  assert.equal(publicError({ code: "throttled", retryAfterMs: 1e12 }).retryAfterMs, 30000);
  assert.equal(publicError({ code: "pin-throttled", retryAfterMs: 2000 }).retryAfterMs, 2000);
  assert.equal(publicError({ code: "throttled", retryAfterMs: Infinity }).retryAfterMs, undefined);
  assert.notEqual(publicError({ code: "invalid-credentials" }).code, publicError({ code: "data-error" }).code);
});
