import assert from "node:assert/strict";
import test from "node:test";
import { installWorker } from "../browser-extension/src/worker.js";
import { mountPopup } from "../browser-extension/src/popup-controller.js";
import { renderView } from "../browser-extension/src/popup-view.js";
import { validateNativeResponse, validView } from "../browser-extension/src/protocol.js";
import { injectFill, inspectForm, inspectFormDiagnostics } from "../browser-extension/src/fill-client.js";
import { browser, port, timers, reply, failure, matches, dom, flush } from "./autofillExtensionFixture.mjs";

const secret = "FAKE-only-sentinel-password";
const docId = "fixture-document-1";
const goodResult = (stage = "combined", kind = stage === "challenge" ? "otp" : null) => [{
  frameId: 0,
  documentId: docId,
  result: kind === null ? { ok: true, stage } : { ok: true, stage, kind },
}];
async function session(stage = "combined") {
  const s = browser(); const clock = timers(); const injections = [];
  s.api.scripting = { async executeScript(options) { injections.push({ ...options, args: structuredClone(options.args) }); return goodResult(stage); } };
  const stop = installWorker(s.api, clock); const popup = s.popup(); s.api.runtime.onConnect.emit(popup);
  popup.onMessage.emit({ version: 1, requestId: "query", type: "refresh" }); await flush();
  assert.equal(popup.sent.at(-1).state, "matches");
  s.api.answer = request => reply(request, "fillPayload", { username: "fake-user", password: secret });
  return { ...s, clock, stop, popup, injections, fill(id = matches[0].credentialId) { popup.onMessage.emit({ version: 1, requestId: "fill-1", type: "fill", credentialId: id }); } };
}

test("explicit Fill preflights permitted frames, revalidates through native, and targets only the captured document", async () => {
  const s = await session(); assert.equal(s.injections.length, 1); assert.equal(s.native.length, 1);
  s.fill(); await flush();
  assert.deepEqual(s.native[1].sent.map(r => r.type), ["requestFill"]);
  assert.equal(s.native[1].sent[0].pageUrl, s.api.current.url);
  assert.equal(s.native[1].sent[0].credentialId, matches[0].credentialId);
  assert.equal(s.injections.length, 3);
  assert.deepEqual(s.injections[1].target, { tabId: 7, allFrames: true });
  assert.equal(s.injections[1].args.length, 5);
  assert.deepEqual(s.injections[1].args.slice(2), [null, null, 2500]);
  assert.ok(!JSON.stringify(s.injections[1]).includes(secret));
  assert.deepEqual(s.injections[2].target, { tabId: 7, documentIds: [docId] });
  assert.equal(s.injections[2].world, "ISOLATED"); assert.equal(s.injections[2].injectImmediately, true);
  assert.equal(s.injections[2].args[2], "combined"); assert.equal(s.injections[2].args[3].password, secret);
  assert.equal(s.popup.sent.at(-1).state, "filled"); assert.ok(s.popup.sent.every(validView));
  assert.ok(!JSON.stringify(s.popup.sent).includes(secret)); assert.equal(s.clock.pending.size, 0); s.stop();
});

test("staged Fill labels the action and strips the unused field before final injection", async () => {
  for (const [stage, label, cleared] of [["username_only", "Fill username", "password"], ["password_only", "Fill password", "username"]]) {
    const s = await session(stage);
    const button = s.popup.sent.at(-1); assert.equal(button.stage, stage);
    const d = dom(); renderView(d.document, button, () => {}); assert.equal(d.nodes.matches.children[0].children[1].textContent, label);
    s.fill(); await flush();
    assert.equal(s.injections.at(-1).args[2], stage);
    assert.equal(s.injections.at(-1).args[3][cleared], "");
    assert.equal(s.popup.sent.at(-1).state, "filled"); assert.equal(s.popup.sent.at(-1).stage, stage); s.stop();
  }
});

test("preflight failure or missing document identity never retrieves a password", async () => {
  for (const result of [
    [{ frameId: 0, documentId: docId, result: { ok: false, code: "NO_LOGIN_FORM" } }],
    [{ frameId: 0, documentId: docId, result: { ok: false, code: "AMBIGUOUS_LOGIN_FORM" } }],
    [{ frameId: 0, result: { ok: true, stage: "combined" } }], [{ frameId: 1, documentId: docId, result: { ok: true, stage: "combined" } }], [],
    [{ frameId: 0, documentId: docId, result: { ok: true, stage: "combined", password: secret } }],
    [{ frameId: 0, documentId: docId, result: { ok: true, stage: "challenge" } }],
    [{ frameId: 0, documentId: docId, result: { ok: true, stage: "challenge", kind: "captcha" } }],
    [{ frameId: 0, documentId: docId, result: { ok: true, stage: "unsupported", diagnostics: {} } }],
  ]) {
    const s = await session(); s.api.scripting.executeScript = async () => result; s.fill(); await flush();
    assert.equal(s.native.length, 1); assert.notEqual(s.popup.sent.at(-1).state, "filled");
    assert.ok(!JSON.stringify(s.popup.sent).includes(secret)); s.stop();
  }
});

test("non-fillable detection stages never retrieve a fill payload", async () => {
  for (const [stage, state, kind] of [["unsupported", "no_login_form"], ["ambiguous", "ambiguous_login_form"],
    ["challenge", "challenge", "otp"], ["challenge", "security_challenge", "website"], ["passwordless", "passwordless"]]) {
    const s = browser(); const clock = timers();
    s.api.scripting.executeScript = async () => goodResult(stage, kind ?? null);
    const stop = installWorker(s.api, clock); const popup = s.popup(); s.api.runtime.onConnect.emit(popup);
    popup.onMessage.emit({ version: 1, requestId: "query", type: "refresh" }); await flush();
    assert.equal(popup.sent.at(-1).state, state); assert.equal(popup.sent.at(-1).stage, stage);
    assert.deepEqual(s.native[0].sent.map(request => request.type), ["status", "queryMatches"]);
    popup.onMessage.emit({ version: 1, requestId: "fill", type: "fill", credentialId: matches[0].credentialId }); await flush();
    assert.equal(s.native.length, 1); stop();
  }
});

test("unoffered, forged and duplicate Fill requests cannot retrieve extra credentials", async () => {
  const forged = await session(); forged.fill("b".repeat(32)); await flush();
  assert.equal(forged.native.length, 1); assert.equal(forged.popup.disconnected, true); forged.stop();
  const s = await session(); s.api.answer = () => undefined;
  s.fill(); s.fill(); await flush(); s.fill(); await flush();
  assert.equal(s.native.length, 2); assert.equal(s.native[1].sent.length, 1);
  s.native[1].onMessage.emit(reply(s.native[1].sent[0], "fillPayload", { username: "fake", password: secret })); await flush();
  s.fill(); await flush(); assert.equal(s.native.length, 2); assert.equal(s.popup.disconnected, true); s.stop();
});

test("lock, changed website and deleted credential refuse Fill without secret injection", async () => {
  for (const [code, state] of [["VAULT_LOCKED", "locked"], ["DOMAIN_MISMATCH", "domain_mismatch"], ["CREDENTIAL_NOT_FOUND", "credential_not_found"]]) {
    const s = await session(); s.api.answer = r => failure(r, code); s.fill(); await flush();
    assert.equal(s.injections.length, 2); assert.equal(s.popup.sent.at(-1).state, state);
    assert.ok(!JSON.stringify(s.popup.sent).includes("sentinel")); s.stop();
  }
});

test("navigation or popup closure while waiting for native prevents injection and discards raw payload refs", async () => {
  for (const invalidate of [s => s.api.tabs.onUpdated.emit(7, { status: "loading" }), s => s.popup.onDisconnect.emit(), s => s.clock.expire(30000)]) {
    const s = await session(); s.api.answer = () => undefined; s.fill(); await flush();
    invalidate(s); s.native[1].onMessage.emit(reply(s.native[1].sent[0], "fillPayload", { username: "fake", password: secret })); await flush();
    assert.equal(s.injections.length, 2); assert.equal(s.native[1].disconnected, true); s.stop();
  }
});

test("changed URL detected after native response prevents injection even without navigation events", async () => {
  const s = await session();
  s.api.answer = r => { s.api.current.url = "https://evil.test"; return reply(r, "fillPayload", { username: "fake", password: secret }); };
  s.fill(); await flush(); assert.equal(s.injections.length, 2); assert.equal(s.popup.sent.at(-1).state, "page_changed"); s.stop();
});

test("payload references clear on cancellation during injection and completion never enters the popup", async () => {
  const s = await session(); let retained; let finish;
  s.api.scripting.executeScript = async options => {
    if (options.args.length === 5) return goodResult();
    retained = options.args[3]; return new Promise(resolve => { finish = resolve; });
  };
  let raw;
  s.api.answer = r => { raw = reply(r, "fillPayload", { username: "fake", password: secret }); return raw; };
  s.fill(); await flush(); assert.equal(retained.password, secret); assert.equal(raw.data.password, "");
  s.popup.onDisconnect.emit(); assert.equal(retained.password, "");
  finish(goodResult()); await flush(); assert.ok(!s.popup.sent.some(x => x.state === "filled")); s.stop();
});

test("every explicit inspection uses a fresh bounded delayed-detection invocation", async () => {
  const calls = [];
  const api = { scripting: { async executeScript(options) { calls.push(options); return goodResult(); } } };
  const page = { tabId: 7, pageUrl: "https://fixture.test/login" };
  await inspectForm(api, page);
  await inspectForm(api, page);
  assert.equal(calls.length, 2);
  assert.notEqual(calls[0].args, calls[1].args);
  for (const call of calls) {
    assert.deepEqual(call.target, { tabId: 7, allFrames: true });
    assert.deepEqual(call.args.slice(2), [null, null, 2500]);
    assert.ok(call.args[1] >= Date.now() + 2500);
  }
});

test("developer diagnostics are explicit, bounded and separate from production inspection", async () => {
  const calls = [];
  const diagnosticResult = [{
    frameId: 0,
    documentId: docId,
    result: {
      ok: true,
      stage: "unsupported",
      kind: "",
      diagnostics: {
        origin: "https://fixture.test",
        counts: { inputs: 1, visibleInputs: 1, usernameCandidates: 0, passwordCandidates: 0,
          otpCandidates: 0, interactiveControls: 0, challengeIndicators: 0 },
        fields: [{ index: 0, type: "search", autocomplete: "", id: "site-search", name: "search",
          visible: true, score: 0, rejectionReason: "unsupported_type" }],
        truncated: false,
        detectedStage: "unsupported",
      },
    },
  }];
  const api = { scripting: { async executeScript(options) { calls.push(options); return diagnosticResult; } } };
  const page = { tabId: 7, pageUrl: "https://fixture.test/login" };
  const report = await inspectFormDiagnostics(api, page);
  assert.deepEqual(Object.keys(report), ["version", "stage", "kind", "selectedFrameId", "selectedDocumentId", "frames"]);
  assert.equal(report.stage, "unsupported"); assert.equal(report.frames[0].origin, "https://fixture.test");
  assert.equal(report.frames[0].fields[0].rejectionReason, "unsupported_type");
  assert.ok(!Object.hasOwn(report.frames[0].fields[0], "value"));
  assert.deepEqual(calls[0].args.slice(2), [null, null, 0, true]);
  api.scripting.executeScript = async options => { calls.push(options); return goodResult("unsupported", null); };
  assert.equal((await inspectForm(api, page)).stage, "unsupported");
  assert.deepEqual(calls[1].args.slice(2), [null, null, 2500]);
});

test("developer diagnostics reject values, unknown reasons and wrong frame origins", async () => {
  const page = { tabId: 7, pageUrl: "https://fixture.test/login" };
  const base = {
    ok: true, stage: "unsupported", kind: "", diagnostics: {
      origin: "https://fixture.test",
      counts: { inputs: 1, visibleInputs: 1, usernameCandidates: 0, passwordCandidates: 0,
        otpCandidates: 0, interactiveControls: 0, challengeIndicators: 0 },
      fields: [{ index: 0, type: "text", autocomplete: "", id: "login", name: "login",
        visible: true, score: 100, rejectionReason: "username_candidate" }],
      truncated: false, detectedStage: "unsupported",
    },
  };
  for (const result of [
    { ...base, diagnostics: { ...base.diagnostics, origin: "https://evil.test" } },
    { ...base, diagnostics: { ...base.diagnostics, fields: [{ ...base.diagnostics.fields[0], value: secret }] } },
    { ...base, diagnostics: { ...base.diagnostics, fields: [{ ...base.diagnostics.fields[0], rejectionReason: secret }] } },
  ]) {
    const api = { scripting: { async executeScript() { return [{ frameId: 0, documentId: docId, result }]; } } };
    await assert.rejects(inspectFormDiagnostics(api, page), error => error.code === "NO_LOGIN_FORM");
  }
});

test("frame inspection selects one same-origin candidate and rejects multiple plausible frames", async () => {
  const page = { tabId: 7, pageUrl: "https://fixture.test/login" };
  const main = { frameId: 0, documentId: docId, result: { ok: true, stage: "unsupported" } };
  const child = { frameId: 4, documentId: "fixture-frame-document", result: { ok: true, stage: "combined" } };
  const crossOrigin = { frameId: 8, documentId: "cross-origin-document", result: { ok: false, code: "PAGE_CHANGED" } };
  const api = { scripting: { async executeScript() { return [main, child, crossOrigin]; } } };
  assert.deepEqual(await inspectForm(api, page), {
    frameId: 4, documentId: "fixture-frame-document", stage: "combined",
  });
  api.scripting.executeScript = async () => [main, child, {
    frameId: 9, documentId: "second-frame-document", result: { ok: true, stage: "password_only" },
  }];
  assert.equal((await inspectForm(api, page)).stage, "ambiguous");
});

test("frame inspection propagates passwordless recognition without making it fillable", async () => {
  const page = { tabId: 7, pageUrl: "https://fixture.test/login" };
  const api = { scripting: { async executeScript() { return [
    { frameId: 0, documentId: docId, result: { ok: true, stage: "unsupported" } },
    { frameId: 4, documentId: "passkey-document", result: { ok: true, stage: "passwordless" } },
  ]; } } };
  assert.deepEqual(await inspectForm(api, page), {
    frameId: 4, documentId: "passkey-document", stage: "passwordless",
  });
});

test("frame inspection preserves the fixed website-challenge kind", async () => {
  const page = { tabId: 7, pageUrl: "https://fixture.test/login" };
  const api = { scripting: { async executeScript() { return [
    { frameId: 0, documentId: docId, result: { ok: true, stage: "challenge", kind: "website" } },
  ]; } } };
  assert.deepEqual(await inspectForm(api, page), {
    frameId: 0, documentId: docId, stage: "challenge", kind: "website",
  });
});

test("Fill targets only the freshly selected same-origin frame document", async () => {
  const s = browser(); const clock = timers(); const injections = [];
  s.api.scripting.executeScript = async options => {
    injections.push({ ...options, args: structuredClone(options.args) });
    if (options.target.allFrames) return [
      { frameId: 0, documentId: docId, result: { ok: true, stage: "unsupported" } },
      { frameId: 4, documentId: "fixture-frame-document", result: { ok: true, stage: "combined" } },
    ];
    return [{ frameId: 4, documentId: "fixture-frame-document", result: { ok: true, stage: "combined" } }];
  };
  const stop = installWorker(s.api, clock); const popup = s.popup(); s.api.runtime.onConnect.emit(popup);
  popup.onMessage.emit({ version: 1, requestId: "frame-query", type: "refresh" }); await flush();
  assert.equal(popup.sent.at(-1).state, "matches");
  s.api.answer = request => reply(request, "fillPayload", { username: "fake-user", password: secret });
  popup.onMessage.emit({ version: 1, requestId: "frame-fill", type: "fill", credentialId: matches[0].credentialId });
  await flush();
  assert.deepEqual(injections.at(-1).target, { tabId: 7, documentIds: ["fixture-frame-document"] });
  assert.equal(injections.at(-1).args[3].password, secret);
  assert.equal(popup.sent.at(-1).state, "filled");
  stop();
});

test("an SPA stage change is detected freshly and stale Fill intent fails before secret retrieval", async () => {
  const s = await session("username_only");
  s.api.scripting.executeScript = async () => goodResult("password_only");
  s.fill(); await flush();
  assert.equal(s.native.length, 1);
  assert.equal(s.popup.sent.at(-1).state, "page_changed");
  assert.ok(!JSON.stringify(s.popup.sent).includes(secret));
  s.stop();
});

test("document-targeted injection rejects wrong-document results and drops arguments after failure", async () => {
  let args;
  const api = { scripting: { async executeScript(options) { args = options.args; throw new Error(secret); } } };
  await assert.rejects(injectFill(api, { tabId: 7, pageUrl: "https://fixture.test/" }, 0, docId, "combined", { username: "fake", password: secret }), e => e.code === "PAGE_CHANGED" && !e.message.includes(secret));
  assert.deepEqual(args, []);
  api.scripting.executeScript = async () => [{ ...goodResult()[0], documentId: "replacement-document" }];
  await assert.rejects(injectFill(api, { tabId: 7, pageUrl: "https://fixture.test/" }, 0, docId, "combined", { username: "fake", password: secret }), e => e.code === "PAGE_CHANGED");
});

test("Fill payload schema stays exclusive to Fill and cannot enter a popup view", () => {
  const request = { version: 1, requestId: "fill", type: "requestFill" };
  const value = reply(request, "fillPayload", { username: "fake", password: secret });
  assert.equal(validateNativeResponse(value, request).password, secret);
  for (const data of [{ username: "fake", password: "" }, { username: "fake", password: secret, key: secret }, { username: "fake", password: "x".repeat(4097) }]) {
    assert.throws(() => validateNativeResponse(reply(request, "fillPayload", data), request));
  }
  for (const type of ["status", "queryMatches"]) assert.throws(() => validateNativeResponse(value, { ...request, type }));
  assert.equal(validView({ version: 1, requestId: "fill", type: "view", state: "filled", host: "fixture.test", matches: [], password: secret }), false);
});

test("only a trusted Fill click sends a selected ID, and pending buttons cannot queue a second request", () => {
  const { api } = browser(); const p = port(); api.runtime.connect = () => p;
  const d = dom(); const clock = timers(); const stop = mountPopup(api, d.document, d.window, clock);
  p.onMessage.emit({ version: 1, requestId: p.sent[0].requestId, type: "view", state: "matches", stage: "combined", host: "fixture.test", matches });
  const fill = d.nodes.matches.children[0].children[1]; assert.equal(fill.disabled, false);
  assert.equal(fill.textContent, "Fill login");
  fill.fire("click", { isTrusted: false }); assert.equal(p.sent.length, 1);
  fill.fire("click"); assert.equal(p.sent.length, 2);
  assert.equal(p.sent[1].type, "fill"); assert.equal(p.sent[1].credentialId, matches[0].credentialId);
  assert.deepEqual(Object.keys(p.sent[1]).sort(), ["credentialId", "requestId", "type", "version"]);
  fill.fire("click"); assert.equal(p.sent.length, 2); assert.equal(d.nodes.retry.disabled, true);
  p.onMessage.emit({ version: 1, requestId: p.sent[1].requestId, type: "view", state: "filled", stage: "combined", host: "fixture.test", matches: [] });
  assert.equal(d.nodes.matches.children.length, 0); assert.match(d.nodes.status.textContent, /filled/); stop();
});
