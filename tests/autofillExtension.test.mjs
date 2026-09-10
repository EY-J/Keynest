import assert from "node:assert/strict";
import test from "node:test";
import { eligiblePage, validateNativeResponse, PublicFailure, validView } from "../browser-extension/src/protocol.js";
import { openNative } from "../browser-extension/src/native-client.js";
import { installWorker } from "../browser-extension/src/worker.js";
import { renderView } from "../browser-extension/src/popup-view.js";
import { mountPopup } from "../browser-extension/src/popup-controller.js";
import { browser, port, timers, reply, failure, matches, dom, flush } from "./autofillExtensionFixture.mjs";

function session() {
  const fixture = browser();
  const clock = timers();
  const stop = installWorker(fixture.api, clock);
  const popup = fixture.popup();
  fixture.api.runtime.onConnect.emit(popup);
  return { ...fixture, clock, popup, stop, refresh(id = "popup-1") { popup.onMessage.emit({ version: 1, requestId: id, type: "refresh" }); } };
}

test("extension URL eligibility uses parsed HTTPS hosts and never repairs unsafe input", () => {
  assert.deepEqual(eligiblePage("https://EXAMPLE.test./login?next=x#f"), { pageUrl: "https://example.test./login?next=x#f", host: "example.test" });
  assert.equal(eligiblePage("https://example.test:8443/").host, "example.test");
  assert.equal(eligiblePage("https://example.test.evil.test").host, "example.test.evil.test");
  assert.equal(eligiblePage("https://bücher.test").host, "xn--bcher-kva.test");
  for (const url of [null, "", "example.test", "http://example.test", "chrome://extensions", "edge://settings", "file:///test", "javascript:alert(1)", "https:example.test", "https:///example.test", "https://a..test", "https://a.test..", "https://user:secret@a.test", "https://a.test/ space", "https://a.test/\\evil", "https://a.test/\n", "https://a.test/" + "x".repeat(8192)]) {
    assert.throws(() => eligiblePage(url), error => error.code === "UNSUPPORTED_URL");
  }
});

test("native replies are correlated, bounded, typed, and cannot smuggle passwords into summaries", () => {
  const request = { version: 1, requestId: "fixture", type: "queryMatches" };
  assert.deepEqual(validateNativeResponse(reply(request, "matches", { matches }), request), { matches });
  const valid = reply(request, "matches", { matches });
  const invalid = [
    [], null, { ...valid, version: 2 }, { ...valid, requestId: "stale" },
    { ...valid, password: "sentinel" }, reply(request, "fillPayload", { username: "u", password: "sentinel" }),
    reply(request, "matches", { matches: [{ ...matches[0], password: "sentinel" }] }),
    reply(request, "matches", { matches: [matches[0], matches[0]] }),
    reply(request, "matches", { matches: [{ ...matches[0], name: "x".repeat(201) }] }),
    reply(request, "matches", { matches: [[matches[0].credentialId, "x", "u"]] }),
    reply(request, "matches", { matches: Array.from({ length: 600 }, (_, i) => ({ ...matches[0], credentialId: i.toString(16).padStart(32, "0"), name: "x".repeat(200) })) }),
  ];
  for (const value of invalid) assert.throws(() => validateNativeResponse(value, request), error => error.code === "IPC_UNAVAILABLE");
  assert.throws(() => validateNativeResponse(failure(request, "VAULT_LOCKED"), request), error => error.code === "VAULT_LOCKED" && !error.message.includes("sentinel"));
  assert.throws(() => validateNativeResponse(failure(request, "private-code"), request), error => error.code === "IPC_UNAVAILABLE");
});

test("host-approval replies are strictly typed and password-free", () => {
  const request = { version: 1, requestId: "approval", type: "requestHostApproval" };
  assert.deepEqual(
    validateNativeResponse(reply(request, "hostApprovalRequested", { state: "requested" }), request),
    { state: "requested" },
  );
  for (const invalid of [
    reply(request, "hostApprovalRequested", { state: "completed" }),
    reply(request, "hostApprovalRequested", { state: "requested", password: "sentinel" }),
    reply(request, "fillPayload", { username: "u", password: "sentinel" }),
  ]) assert.throws(() => validateNativeResponse(invalid, request), error => error.code === "IPC_UNAVAILABLE");
});

test("worker is idle until popup refresh, queries only browser URL, and releases native connection", async () => {
  const s = session();
  assert.equal(s.native.length, 0); assert.equal(s.api.queryCount, 0);
  s.refresh(); await flush();
  assert.equal(s.native.length, 1);
  assert.equal(s.native[0].host, "com.eyy.keynest.autofill");
  assert.deepEqual(s.native[0].sent.map(x => x.type), ["status", "queryMatches"]);
  assert.equal(s.native[0].sent[1].pageUrl, s.api.current.url);
  assert.equal(new Set(s.native[0].sent.map(x => x.requestId)).size, 2);
  assert.deepEqual(s.popup.sent.at(-1).matches, matches);
  assert.equal(s.popup.sent.at(-1).host, "fixture.test");
  assert.equal(s.native[0].disconnected, true); assert.equal(s.clock.pending.size, 0);
  await flush(); assert.equal(s.native.length, 1); // no poll/reconnect
  s.stop(); assert.equal(s.api.tabs.onUpdated.listeners.size, 0);
});

test("worker rejects other extensions, page/content-script senders and forged refresh fields", async () => {
  const s = session();
  for (const sender of [undefined, { id: "evil", url: s.api.runtime.getURL("src/popup.html") }, { id: s.api.runtime.id, url: "https://evil.test" }, { id: s.api.runtime.id, url: s.api.runtime.getURL("src/popup.html"), tab: { id: 1 } }, { id: s.api.runtime.id, url: s.api.runtime.getURL("src/popup.html"), frameId: 1 }]) {
    const p = port(sender); s.api.runtime.onConnect.emit(p); assert.equal(p.disconnected, true);
  }
  s.popup.onMessage.emit({ version: 1, requestId: "forged", type: "refresh", pageUrl: "https://evil.test" });
  await flush(); assert.equal(s.native.length, 0); assert.equal(s.popup.disconnected, true);
  s.stop();
  const fill = session();
  fill.popup.onMessage.emit({ version: 1, requestId: "fill", type: "requestFill" });
  await flush(); assert.equal(fill.native.length, 0); assert.equal(fill.popup.disconnected, true); fill.stop();
});

test("locked, unavailable, unsupported and damaged states do not query summaries", async () => {
  for (const [state, expected] of [["locked", "locked"], ["app_not_running", "unavailable"], ["unsupported", "unsupported"], ["error", "error"]]) {
    const s = session(); s.api.answer = r => reply(r, "status", { state });
    s.refresh(); await flush();
    assert.deepEqual(s.native[0].sent.map(x => x.type), ["status"]);
    assert.equal(s.popup.sent.at(-1).state, expected); assert.deepEqual(s.popup.sent.at(-1).matches, []); s.stop();
  }
});

test("no matches, multiple accounts and lock between status/query have distinct safe views", async () => {
  for (const [result, expected, count] of [
    [r => failure(r, "NO_MATCHES"), "no_matches", 0],
    [r => reply(r, "matches", { matches: [] }), "no_matches", 0],
    [r => failure(r, "VAULT_LOCKED"), "locked", 0],
    [r => reply(r, "matches", { matches: [...matches, { ...matches[0], credentialId: "b".repeat(32) }] }), "matches", 2],
  ]) {
    const s = session(); s.api.answer = r => r.type === "status" ? reply(r, "status", { state: "unlocked" }) : result(r);
    s.refresh(); await flush();
    assert.equal(s.popup.sent.at(-1).state, expected); assert.equal(s.popup.sent.at(-1).matches.length, count);
    assert.ok(!JSON.stringify(s.popup.sent).includes("sentinel")); s.stop();
  }
});

test("no-match Review sends only the current page URL and approval never fills or retries automatically", async () => {
  const s = session();
  s.api.answer = request => {
    if (request.type === "status") return reply(request, "status", { state: "unlocked" });
    if (request.type === "queryMatches") return failure(request, "NO_MATCHES");
    if (request.type === "requestHostApproval") {
      return reply(request, "hostApprovalRequested", { state: "requested" });
    }
    throw new Error(`unexpected operation: ${request.type}`);
  };
  s.refresh(); await flush();
  assert.equal(s.popup.sent.at(-1).state, "no_matches");
  s.popup.onMessage.emit({ version: 1, requestId: "review-1", type: "review" });
  await flush();
  const approval = s.native[1].sent[0];
  assert.deepEqual(Object.keys(approval).sort(), ["pageUrl", "requestId", "type", "version"]);
  assert.equal(approval.type, "requestHostApproval");
  assert.equal(approval.pageUrl, "https://fixture.test/login");
  assert.ok(!JSON.stringify(approval).toLowerCase().includes("password"));
  assert.equal(s.popup.sent.at(-1).state, "approval_requested");
  assert.deepEqual(s.native.flatMap(port => port.sent).map(request => request.type), ["status", "queryMatches", "requestHostApproval"]);
  await flush();
  assert.equal(s.native.length, 2);
  s.stop();
});

test("locked state permits a password-free desktop review request but exposes no summaries", async () => {
  const s = session();
  s.api.answer = request => request.type === "status"
    ? reply(request, "status", { state: "locked" })
    : reply(request, "hostApprovalRequested", { state: "requested" });
  s.refresh(); await flush();
  assert.equal(s.popup.sent.at(-1).state, "locked");
  assert.deepEqual(s.popup.sent.at(-1).matches, []);
  s.popup.onMessage.emit({ version: 1, requestId: "locked-review", type: "review" });
  await flush();
  assert.deepEqual(s.native[1].sent.map(request => request.type), ["requestHostApproval"]);
  assert.equal(s.native[1].sent[0].credentialId, undefined);
  assert.equal(s.popup.sent.at(-1).state, "approval_requested");
  s.stop();
});

test("unsupported or loading tabs never connect to native host", async () => {
  for (const changes of [{ url: undefined }, { url: "http://fixture.test" }, { pendingUrl: "https://other.test" }, { status: "loading" }]) {
    const s = session(); Object.assign(s.api.current, changes); s.refresh(); await flush();
    assert.equal(s.native.length, 0); assert.equal(s.popup.sent.at(-1).state, "unsupported_url"); s.stop();
  }
});

test("navigation, tab switch, removal and focus loss cancel pending requests without a new query", async () => {
  for (const event of [
    s => s.api.tabs.onUpdated.emit(7, { status: "loading" }),
    s => s.api.tabs.onUpdated.emit(7, { url: "https://other.test" }),
    s => s.api.tabs.onActivated.emit({ tabId: 8, windowId: 9 }),
    s => s.api.tabs.onRemoved.emit(7),
    s => s.api.windows.onFocusChanged.emit(-1),
  ]) {
    const s = session(); s.api.answer = () => undefined; s.refresh(); await flush();
    const native = s.native[0]; event(s);
    native.onMessage.emit(reply(native.sent[0], "status", { state: "unlocked" })); await flush();
    assert.equal(s.popup.sent.at(-1).state, "page_changed"); assert.equal(native.disconnected, true);
    assert.equal(native.sent.length, 1); assert.equal(s.clock.pending.size, 0); s.stop();
  }
});

test("fresh active-tab check rejects changed destinations even without a navigation event", async () => {
  const s = session();
  s.api.answer = r => { s.api.current.url = "https://other.test"; return reply(r, "status", { state: "unlocked" }); };
  s.refresh(); await flush();
  assert.equal(s.popup.sent.at(-1).state, "page_changed"); assert.equal(s.native[0].sent.length, 1); s.stop();
});

test("a pending match reply cannot survive same-URL reload, and navigation clears displayed accounts", async () => {
  const s = session();
  s.api.answer = r => r.type === "status" ? reply(r, "status", { state: "unlocked" }) : undefined;
  s.refresh(); await flush();
  assert.equal(s.native[0].sent.length, 2);
  s.api.tabs.onUpdated.emit(7, { status: "loading" });
  s.native[0].onMessage.emit(reply(s.native[0].sent[1], "matches", { matches })); await flush();
  assert.equal(s.popup.sent.at(-1).state, "page_changed"); assert.deepEqual(s.popup.sent.at(-1).matches, []);
  s.api.answer = r => r.type === "status" ? reply(r, "status", { state: "unlocked" }) : reply(r, "matches", { matches });
  s.refresh("retry"); await flush(); assert.equal(s.popup.sent.at(-1).state, "matches");
  s.api.tabs.onUpdated.emit(7, { url: "https://fixture.test/other" });
  assert.equal(s.popup.sent.at(-1).state, "page_changed"); assert.deepEqual(s.popup.sent.at(-1).matches, []);
  s.stop();
});

test("a newly opened popup replaces the prior session and leaves no duplicate browser listeners", async () => {
  const s = session(); s.api.answer = () => undefined; s.refresh(); await flush();
  const second = s.api.runtime.onConnect;
  const replacement = browser().popup(); second.emit(replacement);
  assert.equal(s.popup.disconnected, true); assert.equal(s.native[0].disconnected, true);
  assert.equal(s.api.tabs.onUpdated.listeners.size, 1); assert.equal(s.clock.pending.size, 0);
  s.stop(); assert.equal(s.api.tabs.onUpdated.listeners.size, 0);
});

test("popup disconnect and superseding refresh discard stale replies and remove listeners", async () => {
  const s = session(); s.api.answer = () => undefined; s.refresh(); await flush();
  const first = s.native[0]; s.refresh("popup-2"); await flush();
  assert.equal(first.disconnected, true);
  first.onMessage.emit(reply(first.sent[0], "status", { state: "unlocked" })); await flush();
  assert.equal(s.popup.sent.at(-1).requestId, "popup-2");
  s.popup.onDisconnect.emit(); await flush();
  assert.equal(s.native[1].disconnected, true); assert.equal(s.clock.pending.size, 0);
  assert.equal(s.api.tabs.onUpdated.listeners.size, 0); assert.equal(s.api.windows.onFocusChanged.listeners.size, 0); s.stop();
});

test("native timeout/disconnect/invalid reply and overall query timeout fail safely without retries", async () => {
  for (const action of [
    s => s.clock.expire(12000),
    s => { s.api.runtime.lastError = { message: "sentinel C:/private" }; s.native[0].onDisconnect.emit(); },
    s => s.native[0].onMessage.emit({ password: "sentinel" }),
  ]) {
    const s = session(); s.api.answer = () => undefined; s.refresh(); await flush(); action(s); await flush();
    assert.equal(s.popup.sent.at(-1).state, "unavailable"); assert.ok(!JSON.stringify(s.popup.sent).includes("sentinel"));
    assert.equal(s.clock.pending.size, 0); assert.equal(s.native.length, 1); s.stop();
  }
  const s = session(); let resolve;
  s.api.tabs.query = () => new Promise(r => { resolve = r; });
  s.refresh(); s.clock.expire(30000); resolve([s.api.current]); await flush();
  assert.equal(s.popup.sent.at(-1).state, "unavailable"); assert.equal(s.native.length, 0); s.stop();
});

test("native connector refuses malformed Fill and concurrent requests and handles an already aborted session", async () => {
  const { api, native } = browser(); const clock = timers(); api.answer = () => undefined;
  const controller = new AbortController(); const client = openNative(api, controller.signal, clock);
  await assert.rejects(client.request("requestFill"), error => error.code === "INVALID_REQUEST");
  const pending = client.request("status");
  await assert.rejects(client.request("status"), error => error.code === "IPC_UNAVAILABLE");
  controller.abort(); await assert.rejects(pending, error => error instanceof PublicFailure);
  assert.equal(native[0].disconnected, true); assert.equal(clock.pending.size, 0);
  const stopped = openNative(api, controller.signal, clock);
  await assert.rejects(stopped.request("status")); assert.equal(native[1].disconnected, true);
});

test("popup renders metadata as text with disabled Fill and clears it for every non-match view", () => {
  const { document, nodes } = dom();
  const account = { ...matches[0], name: '<img src=x onerror="sentinel">', username: '<script>sentinel</script>' };
  renderView(document, { state: "matches", stage: "combined", host: "fixture.test", matches: [account, matches[0]] });
  assert.equal(nodes["host-value"].textContent, "fixture.test"); assert.equal(nodes.host.hidden, false);
  assert.equal(nodes.matches.children.length, 2);
  const row = nodes.matches.children[0]; assert.equal(row.children[0].children[0].textContent, account.name);
  assert.equal(row.children[0].children[1].textContent, account.username); assert.equal(row.children[1].disabled, true);
  assert.equal(row.children[1].textContent, "Fill login");
  assert.equal(row.children[1].events.size, 0);
  for (const state of ["loading", "locked", "unavailable", "unsupported_url", "unsupported", "error", "no_matches", "page_changed"]) {
    renderView(document, { state, stage: "", host: "", matches: [] }); assert.equal(nodes.matches.children.length, 0);
    assert.equal(nodes["host-value"].textContent, ""); assert.equal(nodes.host.hidden, true);
    assert.equal(nodes.retry.disabled, state === "loading"); assert.ok(nodes.status.textContent);
  }
});

test("popup exposes Review only for no-match and locked states", () => {
  const { document, nodes } = dom();
  let reviews = 0;
  for (const state of ["no_matches", "locked"]) {
    renderView(document, { state, stage: "", host: "fixture.test", matches: [] }, undefined, () => { reviews++; });
    assert.equal(nodes.review.hidden, false);
    assert.equal(nodes.review.disabled, false);
    nodes.review.fire("click");
  }
  assert.equal(reviews, 2);
  renderView(document, { state: "approval_requested", stage: "", host: "fixture.test", matches: [] });
  assert.equal(nodes.review.hidden, true);
  assert.match(nodes.hint.textContent, /Retry/);
});

test("popup view schema accepts only stage-appropriate states", () => {
  for (const stage of ["combined", "username_only", "password_only"]) {
    assert.equal(validView({ version: 1, requestId: "stage", type: "view", state: "matches", stage, host: "fixture.test", matches }), true);
  }
  assert.equal(validView({ version: 1, requestId: "stage", type: "view", state: "challenge", stage: "challenge", host: "fixture.test", matches: [] }), true);
  assert.equal(validView({ version: 1, requestId: "stage", type: "view", state: "security_challenge", stage: "challenge", host: "fixture.test", matches: [] }), true);
  assert.equal(validView({ version: 1, requestId: "stage", type: "view", state: "passwordless", stage: "passwordless", host: "fixture.test", matches: [] }), true);
  assert.equal(validView({ version: 1, requestId: "stage", type: "view", state: "matches", stage: "challenge", host: "fixture.test", matches }), false);
  assert.equal(validView({ version: 1, requestId: "stage", type: "view", state: "matches", stage: "passwordless", host: "fixture.test", matches }), false);
  assert.equal(validView({ version: 1, requestId: "stage", type: "view", state: "security_challenge", stage: "passwordless", host: "fixture.test", matches: [] }), false);
  assert.equal(validView({ version: 1, requestId: "stage", type: "view", state: "matches", host: "fixture.test", matches }), false);
});

test("popup explains passwordless sign-in without offering a Fill action", () => {
  const { document, nodes } = dom();
  renderView(document, { state: "passwordless", stage: "passwordless", host: "fixture.test", matches: [] }, () => {
    throw new Error("passwordless state must not expose Fill");
  });
  assert.equal(nodes.status.textContent, "Passwordless/passkey sign-in detected.");
  assert.equal(nodes.hint.textContent, "Use the website's sign-in flow, or choose password sign-in manually if offered.");
  assert.equal(nodes.matches.children.length, 0);
});

test("popup explains verification-code steps without offering a Fill action", () => {
  const { document, nodes } = dom();
  renderView(document, { state: "challenge", stage: "challenge", host: "fixture.test", matches: [] }, () => {
    throw new Error("challenge state must not expose Fill");
  });
  assert.equal(nodes.status.textContent, "Verification-code step detected.");
  assert.equal(nodes.hint.textContent, "KeyNest does not fill one-time codes yet.");
  assert.equal(nodes.matches.children.length, 0);
});

test("popup tells users to complete website verification without offering Fill", () => {
  const { document, nodes } = dom();
  renderView(document, { state: "security_challenge", stage: "challenge", host: "fixture.test", matches: [] }, () => {
    throw new Error("security challenge state must not expose Fill");
  });
  assert.equal(nodes.status.textContent, "Complete the website's verification step, then try KeyNest again.");
  assert.equal(nodes.hint.textContent, "");
  assert.equal(nodes.matches.children.length, 0);
});

test("popup requests on open/retry only, ignores stale replies, clears on disconnect and tears down", () => {
  const { api } = browser(); const p = port(); api.runtime.connect = () => p;
  const d = dom(); const clock = timers(); const stop = mountPopup(api, d.document, d.window, clock);
  assert.equal(p.sent.length, 1); d.nodes.retry.fire("click"); assert.equal(p.sent.length, 1);
  const view = { version: 1, requestId: p.sent[0].requestId, type: "view", state: "matches", stage: "combined", host: "fixture.test", matches };
  assert.equal(validView(view), true);
  p.onMessage.emit(view); assert.equal(d.nodes.matches.children.length, 1); assert.equal(clock.pending.size, 0);
  d.nodes.retry.fire("click"); assert.equal(p.sent.length, 2); assert.equal(d.nodes.matches.children.length, 0);
  p.onMessage.emit(view); assert.equal(d.nodes.matches.children.length, 0);
  p.onMessage.emit({ ...view, requestId: p.sent[1].requestId });
  d.document.hidden = true; d.document.fire("visibilitychange");
  assert.equal(p.disconnected, true); assert.equal(d.nodes.matches.children.length, 0); assert.equal(clock.pending.size, 0);
  assert.equal(p.onMessage.listeners.size, 0); stop();
});

test("popup Review is a distinct user action and Retry remains manual", () => {
  const { api } = browser(); const p = port(); api.runtime.connect = () => p;
  const d = dom(); const clock = timers(); const stop = mountPopup(api, d.document, d.window, clock);
  const noMatch = { version: 1, requestId: p.sent[0].requestId, type: "view", state: "no_matches", stage: "", host: "fixture.test", matches: [] };
  p.onMessage.emit(noMatch);
  d.nodes.review.fire("click");
  assert.equal(p.sent.at(-1).type, "review");
  assert.equal(p.sent.at(-1).credentialId, undefined);
  const approval = { ...noMatch, requestId: p.sent.at(-1).requestId, state: "approval_requested" };
  p.onMessage.emit(approval);
  assert.equal(p.sent.length, 2);
  d.nodes.retry.fire("click");
  assert.equal(p.sent.at(-1).type, "refresh");
  stop();
});

test("popup handles missing worker, malformed views and connection loss without showing transport text", () => {
  for (const action of [
    (p, clock) => clock.expire(32000),
    p => p.onMessage.emit({ password: "sentinel" }),
    p => p.onDisconnect.emit(),
  ]) {
    const { api } = browser(); const p = port(); api.runtime.connect = () => p; api.runtime.lastError = { message: "sentinel" };
    const d = dom(); const clock = timers(); const stop = mountPopup(api, d.document, d.window, clock);
    action(p, clock); assert.equal(d.nodes.matches.children.length, 0); assert.match(d.nodes.status.textContent, /unavailable/);
    assert.equal(d.nodes.retry.disabled, false); assert.equal(clock.pending.size, 0); stop();
  }
});
