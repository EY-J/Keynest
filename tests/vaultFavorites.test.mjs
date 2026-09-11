import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import { transformWithOxc } from "vite";

async function credentialFavorites(initialValue = null) {
  let stored = initialValue;
  const file = new URL("../src/features/vault/credentialFavorites.ts", import.meta.url);
  const { code } = await transformWithOxc(await readFile(file, "utf8"), file.pathname);
  const context = vm.createContext({
    window: {
      localStorage: {
        getItem: () => stored,
        setItem: (_key, value) => { stored = value; },
      },
    },
  });
  const module = new vm.SourceTextModule(code, { context });
  await module.link(() => assert.fail("Favorite store must not import dependencies"));
  await module.evaluate();
  return { api: module.namespace, stored: () => stored };
}

test("favorite IDs toggle through one immutable set and persist", async () => {
  const f = await credentialFavorites();
  const first = f.api.toggleFavoriteCredentialId(new Set(), "credential-a");
  assert.deepEqual([...first], ["credential-a"]);
  const second = f.api.toggleFavoriteCredentialId(first, "credential-a");
  assert.deepEqual([...second], []);

  f.api.saveFavoriteCredentialIds(first);
  assert.deepEqual(JSON.parse(f.stored()), ["credential-a"]);
});

test("favorite persistence rejects malformed and oversized identifiers", async () => {
  const malformed = await credentialFavorites("not-json");
  assert.deepEqual([...malformed.api.loadFavoriteCredentialIds()], []);

  const mixed = await credentialFavorites(JSON.stringify(["valid-id", "", 42, "x".repeat(129)]));
  assert.deepEqual([...mixed.api.loadFavoriteCredentialIds()], ["valid-id"]);
});
