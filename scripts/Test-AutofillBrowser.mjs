// Native acceptance tooling only. No TCP listener, personal browser profile, or real credential.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { createInterface } from "node:readline/promises";
import path from "node:path";
import { launchBrowser } from "./autofill-browser-driver.mjs";

const run = promisify(execFile);
const browserName = process.argv[2];
const desktopId = process.argv[3];
assert.ok(["Chrome", "Edge"].includes(browserName) && /^\d+$/.test(desktopId ?? ""));
const manual = process.argv.includes("--manual");
const headless = !manual && !process.argv.includes("--headed");
const browser = await launchBrowser(browserName, { headless });
const manifestDirectory = path.join(browser.profile, "native-host");
let registered = false;
let popup;
let passed = 0;
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const report = name => { passed++; process.stdout.write(`PASS ${browserName}: ${name}\n`); };
async function manualStep(instruction) {
  const input = createInterface({ input: process.stdin, output: process.stdout });
  try { await input.question(`${instruction}\nPress Enter after that action (do not enter credentials here). `); }
  finally { input.close(); }
}
async function ps(script, args) {
  await run("powershell.exe", ["-NoProfile", "-File", path.resolve("scripts", script), ...args], { windowsHide: true, timeout: 20000 });
}
async function desktop(action, requestedHost) {
  const args = ["-Action", action, "-DesktopProcessId", desktopId];
  if (requestedHost) args.push("-RequestedHost", requestedHost);
  await ps("Control-AutofillDesktop.ps1", args);
  await delay(action === "Unlock" ? 3500 : 300);
}
async function evaluate(sessionId, expression) {
  const reply = await browser.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true }, sessionId);
  assert.ok(!reply.exceptionDetails, "Fixture evaluation failed (details suppressed).");
  return reply.result.value;
}
async function until(read, accept, description, timeoutMs = 16000) {
  const deadline = Date.now() + timeoutMs;
  do { const value = await read(); if (accept(value)) return value; await delay(100); } while (Date.now() < deadline);
  throw new Error(`Timed out: ${description}`);
}
async function click(sessionId, selector) {
  if (manual) {
    process.stdout.write((selector.includes("fixture A")
      ? "In the KeyNest popup, click Fill for Autofill fixture A. Verify both fields populate and the website does not submit."
      : "In the KeyNest popup, click a Fill button. Check the displayed refusal and confirm that page fields stay blank.") + "\nKeep the popup open; no terminal input is needed.\n");
    await until(() => evaluate(sessionId, 'document.getElementById("status").textContent'),
      value => value !== "Saved logins for:" && !value.startsWith("Filling"), "human Fill action", 120000);
    return;
  }
  const point = await evaluate(sessionId, `(() => { const e = document.querySelector(${JSON.stringify(selector)}); if (!e || e.disabled) return null; const r = e.getBoundingClientRect(); return {x:r.x+r.width/2,y:r.y+r.height/2}; })()`);
  assert.ok(point, "Expected enabled fixture button.");
  await browser.send("Input.dispatchMouseEvent", { type: "mousePressed", button: "left", clickCount: 1, ...point }, sessionId);
  await browser.send("Input.dispatchMouseEvent", { type: "mouseReleased", button: "left", clickCount: 1, ...point }, sessionId);
}
const networkViolations = [];
let secretLog = false;
let listenerFailure = false;
const monitored = new Set();
async function monitor(sessionId) {
  monitored.add(sessionId);
  await browser.send("Runtime.enable", {}, sessionId);
  await browser.send("Log.enable", {}, sessionId);
  await browser.send("Network.enable", {}, sessionId);
}
browser.listeners.add(message => {
  if (!monitored.has(message.sessionId)) return;
  if (message.method === "Network.requestWillBeSent" && /^(https?|wss?):/i.test(message.params.request.url)) networkViolations.push(true);
  if (["Runtime.consoleAPICalled", "Runtime.exceptionThrown", "Log.entryAdded"].includes(message.method)
    && /KeyNest-Autofill-Fixture-Password-Only|copper-planet-72-MINT/.test(JSON.stringify(message.params))) secretLog = true;
});
try {
  const version = await browser.send("Browser.getVersion");
  process.stdout.write(`${browserName} native acceptance: ${version.product}\n`);
  const { id } = await browser.send("Extensions.loadUnpacked", { path: path.resolve("browser-extension"), enableInIncognito: false });
  process.stdout.write(`Actual unpacked extension ID: ${id}\n`);
  await ps("register-autofill-host.ps1", ["-Browser", browserName, "-ExtensionId", id,
    "-HostExePath", path.resolve("src-tauri/target/autofill-acceptance/debug/keynest-native-host.exe"), "-ManifestDirectory", manifestDirectory]);
  registered = true;
  const targets = (await browser.send("Target.getTargets", { filter: [{}] })).targetInfos;
  const page = targets.find(target => target.type === "page" && target.url === "about:blank");
  const tab = targets.find(target => target.type === "tab");
  assert.ok(page && tab);
  const { sessionId: pageSession } = await browser.send("Target.attachToTarget", { targetId: page.targetId, flatten: true });
  await browser.send("Page.enable", {}, pageSession);
  let formKind = "normal";
  const html = () => `<!doctype html><html><head><title>Disposable KeyNest acceptance</title></head><body>
    <form id="login">${formKind === "empty" ? "" : '<label>Email <input id="user" type="email" autocomplete="username"></label>'}
    ${formKind === "missing" || formKind === "empty" ? "" : '<label>Password <input id="pass" type="password" autocomplete="current-password"></label>'}
    <button type="submit">Log in</button></form>
    ${formKind === "ambiguous" ? '<form id="login-2"><label>Email <input id="user-2" type="email" autocomplete="username"></label><label>Password <input id="pass-2" type="password" autocomplete="current-password"></label><button type="submit">Log in</button></form>' : ""}
    <script>window.submissions=0;window.events=[];document.addEventListener('submit',e=>{e.preventDefault();window.submissions++});
    for(const type of ['input','change'])document.addEventListener(type,e=>window.events.push(e.target.id+':'+type));</script></body></html>`;
  browser.listeners.add(message => {
    if (message.method !== "Fetch.requestPaused" || message.sessionId !== pageSession) return;
    const { requestId, request } = message.params;
    // All page traffic is intercepted locally; no certificate/network acceptance is implied.
    const allowed = /^https:\/\/(login\.keynest-autofill\.test|login\.keynest-autofill\.test\.evil\.test|other\.keynest-autofill\.test)\//.test(request.url);
    browser.send("Fetch.fulfillRequest", { requestId, responseCode: allowed ? 200 : 404,
      responseHeaders: [{ name: "Content-Type", value: "text/html; charset=utf-8" }], body: Buffer.from(allowed ? html() : "").toString("base64") }, pageSession)
      .catch(() => { listenerFailure = true; });
  });
  await browser.send("Fetch.enable", { patterns: [{ urlPattern: "*", requestStage: "Request" }] }, pageSession);
  async function closePopup() {
    if (popup) {
      const oldId = popup.targetId;
      try { await browser.send("Target.closeTarget", { targetId: oldId }); } catch { /* already closed by the browser */ }
      popup = undefined;
      await until(async () => !(await browser.send("Target.getTargets")).targetInfos.some(t => t.targetId === oldId), Boolean, "previous popup closure");
    }
  }
  async function navigate(host = "login.keynest-autofill.test", kind = "normal") {
    await closePopup(); formKind = kind;
    const url = `https://${host}/login?fixture=${Date.now()}`;
    await browser.send("Page.navigate", { url }, pageSession);
    await until(() => evaluate(pageSession, `location.href === ${JSON.stringify(url)} && document.readyState === 'complete' && !!document.getElementById('login')`), Boolean, "fixture navigation");
    await delay(200);
  }
  async function openPopup() {
    await closePopup();
    await browser.send("Page.bringToFront", {}, pageSession);
    if (manual) {
      process.stdout.write(`ACTION: In the disposable ${browserName} window, click KeyNest Autofill in the Extensions toolbar/menu. Keep its popup open; no terminal input is needed.\n`);
    } else await browser.send("Extensions.triggerAction", { id, targetId: tab.targetId });
    const info = await until(async () => (await browser.send("Target.getTargets")).targetInfos.find(t => t.url === `chrome-extension://${id}/src/popup.html`), Boolean, "real action popup", manual ? 120000 : 16000);
    const attached = await browser.send("Target.attachToTarget", { targetId: info.targetId, flatten: true });
    popup = { targetId: info.targetId, sessionId: attached.sessionId };
    await monitor(popup.sessionId);
    const worker = (await browser.send("Target.getTargets")).targetInfos.find(t => t.type === "service_worker" && t.url.startsWith(`chrome-extension://${id}/`));
    if (worker && !monitored.has(worker.targetId)) {
      const attachedWorker = await browser.send("Target.attachToTarget", { targetId: worker.targetId, flatten: true });
      monitored.add(worker.targetId); await monitor(attachedWorker.sessionId);
    }
  }
  async function status(expected) {
    try { await until(() => evaluate(popup.sessionId, 'document.getElementById("status").textContent'), value => value === expected, expected); }
    catch (error) {
      // Status is fixed product text; never print match rows, native messages or page values.
      process.stdout.write(`Observed popup status: ${await evaluate(popup.sessionId, 'document.getElementById("status").textContent')}\n`);
      throw error;
    }
  }
  const blank = () => evaluate(pageSession, "(!document.getElementById('user') || document.getElementById('user').value === '') && (!document.getElementById('pass') || document.getElementById('pass').value === '') && window.submissions === 0");

  if (process.argv.includes("--unavailable")) {
    await navigate(); await openPopup(); await status("KeyNest desktop app is unavailable.");
    assert.ok(await blank()); report("native host safely reports absent desktop without modifying fields");
  } else {
    await desktop("Lock");
    await navigate(); await openPopup(); await status("KeyNest is locked.");
    assert.ok(await blank()); report("running locked desktop refuses query");
    await desktop("Unlock");
    await openPopup(); await status("Saved logins for:");
    assert.equal(await evaluate(popup.sessionId, "document.querySelectorAll('#matches button').length"), 2);
    assert.ok(await blank()); report("two exact-host summaries; no automatic fill");
    await click(popup.sessionId, '#matches button[aria-label="Fill login for Autofill fixture A"]');
    await status("Login filled.");
    assert.ok(await evaluate(pageSession, "document.getElementById('user').value === 'fixture-A@example.test' && document.getElementById('pass').value === 'KeyNest-Autofill-Fixture-Password-Only-A' && window.submissions === 0 && document.getElementById('pass').getAttribute('value') === null && ['user:input','user:change','pass:input','pass:change'].every(e=>window.events.includes(e))"));
    assert.ok(await evaluate(popup.sessionId, "!document.documentElement.outerHTML.includes('KeyNest-Autofill-Fixture-Password-Only')"));
    report("trusted Fill crosses Native Messaging and desktop pipe; fields/events correct, no submit or password markup/popup");
    assert.ok(await evaluate(popup.sessionId, "typeof chrome.storage === 'undefined'"));
    assert.ok(await evaluate(popup.sessionId, "(async()=>localStorage.length===0 && sessionStorage.length===0 && (await indexedDB.databases()).length===0 && (await caches.keys()).length===0)()"));
    report("chrome.storage unavailable without permission; Web Storage, IndexedDB and Cache Storage empty");

    await navigate(); await openPopup(); await status("Saved logins for:");
    if (manual) await closePopup(); // Real headed action popups close when desktop lock takes focus.
    await desktop("Lock");
    if (manual) await openPopup();
    else await click(popup.sessionId, '#matches button');
    await status("KeyNest is locked.");
    assert.ok(await blank()); report(manual
      ? "reopening the real action after desktop lock reports locked and leaves fields blank"
      : "lock after query refuses stale Fill without modifying fields");
    await desktop("Unlock");
    for (const host of ["login.keynest-autofill.test.evil.test", "other.keynest-autofill.test"]) {
      await navigate(host); await openPopup(); await status("No approved login is available for:");
      assert.equal(await evaluate(popup.sessionId, "document.querySelectorAll('#matches button').length"), 0);
      assert.ok(await blank()); report(`no credentials offered for ${host}`);
      if (host === "other.keynest-autofill.test") {
        await click(popup.sessionId, "#review");
        await desktop("InspectApproval", host);
        await desktop("CancelApproval");
        assert.ok(await blank());
        report("Review crosses the current native host and pipe and opens the exact-host desktop modal");
      }
    }
    for (const [kind, expected] of [["ambiguous", "More than one login field or form could match."], ["empty", "No supported login form found."]]) {
      await navigate("login.keynest-autofill.test", kind); await openPopup(); await status(expected);
      assert.ok(await blank()); report(`${kind} form fails closed`);
    }
    assert.equal(networkViolations.length, 0); assert.equal(secretLog, false); assert.equal(listenerFailure, false);
    report("monitored extension targets made no HTTP/WebSocket requests and emitted no fixture secrets to console/log events");
    if (manual) await manualStep("Before cleanup, review extension DevTools Network, storage and logs as described in docs/release/browser-autofill-dev-setup.md. Record pass/fail only; do not copy secrets or native payloads. Report any unverified review as pending.");
    await desktop("Lock");
  }
  process.stdout.write(`${browserName}: ${passed} native checks passed (${manual ? "human-assisted" : headless ? "headless, tool-driven" : "headed, tool-driven"}). Human acceptance/review requires the tester's reported results.\n`);
} finally {
  process.stdout.write(`Browser diagnostic metadata: ${JSON.stringify(browser.diagnostics())}\n`);
  // Remove only this run's exact registration before deleting its temporary profile.
  if (registered) await ps("unregister-autofill-host.ps1", ["-Browser", browserName, "-ManifestDirectory", manifestDirectory]);
  await browser.close();
}
