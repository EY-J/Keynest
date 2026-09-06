import assert from "node:assert/strict";
import test from "node:test";
import { readFile } from "node:fs/promises";
const read = path => readFile(new URL(`../${path}`, import.meta.url), "utf8");
test("optional signing override uses installed Tauri fields, without affecting development", async () => {
  const template = JSON.parse(await read("docs/release/tauri.signing.example.json"));
  const schema = JSON.parse(await read("node_modules/@tauri-apps/cli/config.schema.json"));
  const config = JSON.parse(await read("src-tauri/tauri.conf.json"));
  for (const [name, value] of Object.entries(template.bundle.windows)) {
    const field = schema.definitions.WindowsConfig.properties[name];
    assert.ok(field, name); assert.ok([field.type].flat().includes(typeof value));
  }
  assert.equal(template.bundle.windows.digestAlgorithm, "sha256");
  assert.equal(template.bundle.windows.tsp, true);
  assert.ok(!config.bundle.windows?.certificateThumbprint);
  assert.ok(!config.bundle.windows?.signCommand);
});
test("manual release strategy introduces no updater or background update permissions", async () => {
  for (const path of ["package.json", "src-tauri/Cargo.toml", "src-tauri/capabilities/default.json", "src-tauri/tauri.conf.json"]) {
    assert.doesNotMatch(await read(path), /tauri-plugin-updater|@tauri-apps\/plugin-updater|updater:/);
  }
});
