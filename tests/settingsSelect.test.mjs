import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("only the Auto Lock and Clipboard selects use the Settings time width", async () => {
  const source = await readFile(
    new URL("../src/features/settings/components/SecuritySettings.tsx", import.meta.url),
    "utf8",
  );
  assert.equal(source.match(/className="settings-time-select"/g)?.length, 2);
  assert.equal(source.match(/menuClassName="settings-time-select-menu"/g)?.length, 2);
  assert.match(source, /id="auto-lock-seconds"[\s\S]*?className="settings-time-select"/);
  assert.match(source, /id="clipboard-clear-seconds"[\s\S]*?className="settings-time-select"/);
});

test("Settings time selects and their portal menus stay wide and on one line", async () => {
  const [css, selectSource] = await Promise.all([
    readFile(new URL("../src/styles/globals.css", import.meta.url), "utf8"),
    readFile(new URL("../src/components/ui/Select.tsx", import.meta.url), "utf8"),
  ]);

  assert.match(css, /\.settings-row \.keynest-select\.settings-time-select\s*\{[^}]*width:\s*160px;[^}]*min-width:\s*min\(150px,\s*100%\);[^}]*max-width:\s*100%;/s);
  assert.match(css, /\.settings-time-select[\s\S]*?white-space:\s*nowrap;/);
  assert.match(css, /\.settings-time-select-menu \.keynest-select__option\s*\{[^}]*display:\s*flex;[^}]*align-items:\s*center;[^}]*justify-content:\s*space-between;[^}]*gap:\s*12px;[^}]*white-space:\s*nowrap;/s);
  assert.match(css, /\.settings-time-select-menu \.keynest-select__option > span\s*\{[^}]*flex:\s*1;[^}]*white-space:\s*nowrap;/s);
  assert.match(css, /\.settings-time-select-menu \.keynest-select__option > svg\s*\{[^}]*flex-shrink:\s*0;/s);
  assert.match(selectSource, /const width = Math\.min\(rect\.width, viewportWidth - 16\);/);
  assert.match(selectSource, /style=\{\{[\s\S]*?width: position\.width,/);
});
