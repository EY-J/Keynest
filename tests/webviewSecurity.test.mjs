import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const config = JSON.parse(await readFile(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const csp = Object.fromEntries(config.app.security.csp.split(";").map(s => s.trim().split(/\s+/)).map(([name, ...sources]) => [name, sources]));
test("production CSP allows only bundled executable/assets and explicit Tauri IPC", () => {
  assert.deepEqual(csp["default-src"], ["'self'"]);
  assert.deepEqual(csp["script-src"], ["'self'"]);
  assert.deepEqual(csp["font-src"], ["'self'"]);
  assert.deepEqual(csp["connect-src"], ["ipc:", "http://ipc.localhost"]);
  for (const directive of ["object-src", "frame-src", "worker-src", "base-uri", "form-action"]) assert.deepEqual(csp[directive], ["'none'"]);
  assert.ok(!config.app.security.csp.includes("unsafe-eval"));
  assert.ok(!config.app.security.dangerousDisableAssetCspModification);
});
test("Vite production HTML uses bundled scripts/styles/images, with no inline scripts", async () => {
  const html = await readFile(new URL("../dist/index.html", import.meta.url), "utf8");
  for (const [, source] of html.matchAll(/(?:src|href)="([^"]+)"/g)) assert.match(source, /^\/assets\//);
  assert.doesNotMatch(html, /<script(?![^>]*src=)[^>]*>/);
  assert.doesNotMatch(html, /https?:|<iframe|<base/);
});
