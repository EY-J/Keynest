// Isolation probe only: about:blank, no registration, vault requests, or real site.
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { launchBrowser } from "./autofill-browser-driver.mjs";

const browserName = process.argv[2];
const stage = process.argv.find(arg => arg.startsWith("--stage="))?.slice(8) ?? "action";
assert.ok(["launch", "load", "popup", "action"].includes(stage));
const minimal = process.argv.includes("--minimal");
const browser = await launchBrowser(browserName, { headless: !process.argv.includes("--headed") });
let failed = false;
let currentStep = "launch";
try {
  const version = await browser.send("Browser.getVersion");
  process.stdout.write(`PASS launch: ${version.product}; browser PID ${browser.processId}; Node PID ${process.pid}\n`);
  if (stage !== "launch") {
    let extensionPath = path.resolve("browser-extension");
    if (minimal) {
      extensionPath = path.join(browser.profile, "minimal-action-fixture");
      await mkdir(extensionPath);
      await writeFile(path.join(extensionPath, "manifest.json"), JSON.stringify({ manifest_version: 3,
        name: "Disposable action probe", version: "1.0", action: { default_popup: "popup.html" } }));
      await writeFile(path.join(extensionPath, "popup.html"), '<!doctype html><title>Action fixture</title><p id="probe">Local fixture only</p>');
    }
    currentStep = "load";
    const loaded = await browser.send("Extensions.loadUnpacked", { path: extensionPath, enableInIncognito: false });
    process.stdout.write(`PASS extension load (${minimal ? "minimal, no permissions/native code" : "unchanged KeyNest"}): actual ID ${loaded.id}\n`);
    if (["popup", "action"].includes(stage)) {
      const popupPath = minimal ? "popup.html" : "src/popup.html";
      const url = `chrome-extension://${loaded.id}/${popupPath}`;
      if (stage === "popup") {
        currentStep = "direct popup document";
        // Deliberately not an action or activeTab/Native Messaging acceptance test.
        await browser.send("Target.createTarget", { url });
      } else {
        const { targetInfos } = await browser.send("Target.getTargets", { filter: [{}] });
        const tab = targetInfos.find(target => target.type === "tab");
        assert.ok(tab);
        currentStep = "Extensions.triggerAction";
        process.stdout.write("BEGIN Extensions.triggerAction (no host registration or desktop involved)\n");
        await browser.send("Extensions.triggerAction", { id: loaded.id, targetId: tab.targetId });
      }
      await new Promise(resolve => setTimeout(resolve, 700));
      const { targetInfos } = await browser.send("Target.getTargets");
      const popup = targetInfos.find(target => target.url === url);
      assert.ok(popup, "Expected popup document target.");
      const { sessionId } = await browser.send("Target.attachToTarget", { targetId: popup.targetId, flatten: true });
      const { result } = await browser.send("Runtime.evaluate", { expression: minimal
        ? "document.getElementById('probe')?.textContent === 'Local fixture only'"
        : "!!document.getElementById('status') && !document.getElementById('status').textContent.includes('Checking')", returnByValue: true }, sessionId);
      assert.equal(result.value, true);
      process.stdout.write(`PASS ${stage === "popup" ? "direct popup rendering (not action acceptance)" : "action popup rendering"}\n`);
    }
  }
  await browser.send("Browser.getVersion");
  process.stdout.write("PASS browser still responds\n");
} catch {
  failed = true;
  process.stdout.write(`FAIL stage: ${currentStep}; Node caught browser/probe failure and reached cleanup\n`);
} finally {
  process.stdout.write(`Diagnostic metadata before cleanup: ${JSON.stringify(browser.diagnostics())}\n`);
  await browser.close();
  process.stdout.write("PASS disposable profile cleanup; probe Node process is still running\n");
}
if (failed) process.exitCode = 1;
