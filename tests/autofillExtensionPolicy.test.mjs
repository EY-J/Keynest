import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../browser-extension/", import.meta.url);
const manifest = JSON.parse(await readFile(new URL("manifest.json", root), "utf8"));

test("Autofill manifest is MV3 with exact permissions and restrictive CSP", () => {
  assert.equal(manifest.manifest_version, 3);
  assert.equal(manifest.minimum_chrome_version, "106");
  assert.deepEqual(manifest.permissions, ["activeTab", "scripting", "nativeMessaging"]);
  for (const key of ["host_permissions", "optional_host_permissions", "optional_permissions", "content_scripts", "externally_connectable", "web_accessible_resources", "sandbox", "key", "update_url"]) assert.equal(manifest[key], undefined);
  assert.deepEqual(manifest.background, { service_worker: "src/service-worker.js", type: "module" });
  assert.equal(manifest.action.default_popup, "src/popup.html");
  assert.deepEqual(manifest.icons, {
    16: "icons/keynest-16.png",
    32: "icons/keynest-32.png",
    48: "icons/keynest-48.png",
    128: "icons/keynest-128.png",
  });
  assert.deepEqual(manifest.action.default_icon, {
    16: "icons/keynest-16.png",
    32: "icons/keynest-32.png",
  });
  const csp = Object.fromEntries(manifest.content_security_policy.extension_pages.split(";").map(x => x.trim()).filter(Boolean).map(x => { const [key, ...values] = x.split(/\s+/); return [key, values]; }));
  for (const key of ["default-src", "object-src", "connect-src", "base-uri", "form-action", "frame-src"]) assert.deepEqual(csp[key], ["'none'"]);
  for (const key of ["script-src", "style-src", "img-src"]) assert.deepEqual(csp[key], ["'self'"]);
  assert.doesNotMatch(JSON.stringify(manifest), /<all_urls>|unsafe-|https?:|\*/);
});

test("all extension executable/assets have no network, persistence, logging, remote imports or submission", async () => {
  const files = await readdir(new URL("src/", root), { recursive: true });
  for (const file of files) {
    assert.match(file, /\.(js|html|css)$/);
    const source = await readFile(new URL(`src/${file}`, root), "utf8");
    assert.doesNotMatch(source, /\bfetch\s*\(|XMLHttpRequest|WebSocket|EventSource|sendBeacon/);
    assert.doesNotMatch(source, /\b(localStorage|sessionStorage|indexedDB|caches)\b|\.storage\b/);
    assert.doesNotMatch(source, /\bconsole\s*\.|\beval\s*\(|new\s+Function\b|innerHTML|outerHTML|insertAdjacentHTML/);
    assert.doesNotMatch(source, /requestSubmit|\.submit\s*\(|\.click\s*\(|KeyboardEvent|setInterval/);
    if (file !== "fill-client.js") assert.doesNotMatch(source, /\bexecuteScript\s*\(/);
    if (!["worker.js", "native-client.js", "protocol.js"].includes(file)) assert.doesNotMatch(source, /requestFill/);
    if (file === "fill.js") {
      assert.doesNotMatch(source, /\bchrome\b|connectNative|sendMessage|addEventListener|setAttribute/);
      assert.match(source, /Math\.min\(2500,/);
      assert.match(source, /observer\?\.disconnect\(\)/);
      assert.doesNotMatch(source, /setInterval|attachShadow|\b(?:Element|ShadowRoot)\.prototype/);
    }
    for (const [, path] of source.matchAll(/(?:\bfrom\s+|\bimport\s*)["']([^"']+)["']/g)) assert.match(path, /^\.\/[a-z-]+\.js$/);
    if (file.endsWith(".css")) assert.doesNotMatch(source, /@import|url\s*\(/i);
    if (file.endsWith(".html")) {
      assert.doesNotMatch(source, /<script(?![^>]*src=)[^>]*>|\son\w+\s*=|<iframe|<form|<base|\sstyle=/i);
      for (const [, path] of source.matchAll(/(?:src|href)="([^"]+)"/g)) assert.match(path, /^(?:[a-z-]+\.(?:js|css)|\.\.\/icons\/keynest-128\.png)$/);
    }
  }
});
