import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import { transformWithOxc } from "vite";

async function favoriteStore(initialValue = null) {
  let stored = initialValue;
  const file = new URL("../src/features/vault/favoriteStore.ts", import.meta.url);
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
  const f = await favoriteStore();
  const first = f.api.toggledFavoriteRecordIds(new Set(), "record-a");
  assert.deepEqual([...first], ["record-a"]);
  const second = f.api.toggledFavoriteRecordIds(first, "record-a");
  assert.deepEqual([...second], []);

  f.api.saveFavoriteRecordIds(first);
  assert.deepEqual(JSON.parse(f.stored()), ["record-a"]);
});

test("favorite persistence rejects malformed and oversized identifiers", async () => {
  const malformed = await favoriteStore("not-json");
  assert.deepEqual([...malformed.api.loadFavoriteRecordIds()], []);

  const mixed = await favoriteStore(JSON.stringify(["valid-id", "", 42, "x".repeat(129)]));
  assert.deepEqual([...mixed.api.loadFavoriteRecordIds()], ["valid-id"]);
});
