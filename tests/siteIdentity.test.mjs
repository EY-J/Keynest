import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import { transformWithOxc } from "vite";

async function loadSiteIdentity() {
  const file = new URL("../src/utils/serviceIdentity.ts", import.meta.url);
  const source = await readFile(file, "utf8");
  const { code } = await transformWithOxc(source, file.pathname);
  const context = vm.createContext({ URL });
  const module = new vm.SourceTextModule(code, { context });

  await module.link((specifier) => {
    assert.match(specifier, /^\.\.\/assets\/site-icons\/[a-z]+\.svg$/);
    return new vm.SyntheticModule(["default"], function setIconUrl() {
      this.setExport("default", specifier);
    }, { context });
  });
  await module.evaluate();
  return module.namespace;
}

test("recognized credential websites resolve to local service identities", async () => {
  const { getServiceKeyFromUrl } = await loadSiteIdentity();
  const samples = [
    ["https://www.microsoft.com/en-ph", "microsoft"],
    ["https://gmail.com", "google"],
    ["https://accounts.google.com", "google"],
    ["https://facebook.com", "facebook"],
    ["https://github.com", "github"],
    ["https://chatgpt.com", "openai"],
  ];

  for (const [website, expected] of samples) {
    assert.equal(getServiceKeyFromUrl(website), expected);
  }
});

test("unknown and lookalike domains keep the monogram fallback", async () => {
  const { getServiceKeyFromUrl } = await loadSiteIdentity();
  for (const website of [
    "https://my-school-portal.edu",
    "https://notgithub.com",
    "https://google.com.example.test",
    "invalid website",
    "",
  ]) {
    assert.equal(getServiceKeyFromUrl(website), null);
  }
});

test("recognized services use local color images without CSS recoloring", async () => {
  const component = await readFile(
    new URL("../src/components/ui/ServiceLogo.tsx", import.meta.url),
    "utf8",
  );
  const css = await readFile(new URL("../src/styles/globals.css", import.meta.url), "utf8");
  const serviceStyles = css.slice(
    css.indexOf(".service-icon {"),
    css.indexOf(".vault-card-copy {"),
  );

  assert.match(component, /<img className="service-icon__brand"/);
  assert.doesNotMatch(component, /service-icon-color|service-icon-url/);
  assert.doesNotMatch(serviceStyles, /mask:|filter:|currentColor/);
  assert.match(serviceStyles, /object-fit: contain/);
  assert.match(serviceStyles, /service-icon--known[\s\S]*background: transparent/);

  for (const filename of ["google.svg", "microsoft.svg", "instagram.svg", "facebook.svg"]) {
    const asset = await readFile(
      new URL(`../src/assets/site-icons/${filename}`, import.meta.url),
      "utf8",
    );
    assert.match(asset, /#[0-9a-f]{3,8}/i);
  }
});
