import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("all Settings categories provide a shared title and subtitle", async () => {
  const source = await readFile(
    new URL("../src/features/settings/SettingsPage.tsx", import.meta.url),
    "utf8",
  );

  for (const title of ["Profile", "Security", "General", "Appearance", "About"]) {
    assert.match(source, new RegExp(`label: "${title}"`));
  }
  for (const subtitle of [
    "Manage your local profile.",
    "Manage how KeyNest protects your vault.",
    "Configure basic KeyNest behavior.",
    "Choose how KeyNest looks.",
    "KeyNest application information.",
  ]) {
    assert.match(source, new RegExp(subtitle.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
  assert.match(source, /<header className="settings-section-header">[\s\S]*?<h1>\{category\.label\}<\/h1>[\s\S]*?category\.description/);
});

test("the shared Settings heading typography is compact and consistent", async () => {
  const css = await readFile(new URL("../src/styles/globals.css", import.meta.url), "utf8");

  assert.match(css, /\.settings-section-header\s*\{[^}]*margin-bottom:\s*20px;/s);
  assert.match(css, /\.settings-section-header h1\s*\{[^}]*margin:\s*0;[^}]*font-size:\s*28px;[^}]*font-weight:\s*700;[^}]*line-height:\s*1\.1;/s);
  assert.match(css, /\.settings-section-header > p:last-child\s*\{[^}]*margin:\s*7px 0 0;[^}]*color:\s*var\(--kn-muted\);[^}]*font-size:\s*14px;[^}]*line-height:\s*1\.4;[^}]*opacity:\s*0\.7;/s);
  assert.doesNotMatch(css, /#[a-z-]+-panel \.settings-section-header\s*\{/);
});
