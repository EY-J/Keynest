import { FILL_STAGES, POPUP_PORT, PublicFailure, credentialId, eligiblePage, errorState, exact, releaseFill, requestId } from "./protocol.js";
import { openNative } from "./native-client.js";
import { inspectForm, injectFill } from "./fill-client.js";

export function installWorker(api, timers = globalThis) {
  let closePrevious = () => {};
  function onConnect(port) {
    const sender = port.sender;
    if (port.name !== POPUP_PORT || sender?.id !== api.runtime.id
      || sender.url !== api.runtime.getURL("src/popup.html")
      || sender.tab !== undefined || (sender.frameId !== undefined && sender.frameId !== 0)) {
      port.disconnect();
      return;
    }
    closePrevious();
    closePrevious = attachPopup(api, port, timers);
  }
  api.runtime.onConnect.addListener(onConnect);
  return () => { closePrevious(); api.runtime.onConnect.removeListener(onConnect); };
}

function attachPopup(api, port, timers) {
  let closed = false;
  let job = null;
  let page = null;
  let id = null;
  let canReview = false;
  const offeredIds = new Set();
  function cancel() {
    if (!job) return;
    job.controller.abort();
    timers.clearTimeout(job.timer);
    job = null;
  }
  function send(state, host = "", matches = [], stage = "") {
    if (closed || !id) return;
    try { port.postMessage({ version: 1, requestId: id, type: "view", state, stage, host, matches }); }
    catch { dispose(); }
  }
  function invalidate() {
    cancel();
    page = null;
    canReview = false;
    offeredIds.clear();
    send("page_changed");
  }
  async function activePage() {
    const tabs = await api.tabs.query({ active: true, lastFocusedWindow: true });
    const tab = tabs.length === 1 ? tabs[0] : null;
    if (!tab || !Number.isInteger(tab.id) || tab.id < 0 || !Number.isInteger(tab.windowId)
      || tab.pendingUrl || tab.status === "loading") throw new PublicFailure("UNSUPPORTED_URL");
    return { ...eligiblePage(tab.url), tabId: tab.id, windowId: tab.windowId };
  }
  async function checkCurrent(current, snapshot) {
    const fresh = await activePage();
    if (closed || job !== current || fresh.tabId !== snapshot.tabId
      || fresh.windowId !== snapshot.windowId || fresh.pageUrl !== snapshot.pageUrl) {
      throw new PublicFailure("PAGE_CHANGED");
    }
  }
  async function refresh(current) {
    let native;
    let host = "";
    try {
      const snapshot = await activePage();
      if (closed || job !== current) return;
      page = snapshot;
      host = snapshot.host;
      native = openNative(api, current.controller.signal, timers);
      const status = await native.request("status");
      await checkCurrent(current, snapshot);
      if (status.state !== "unlocked") {
        canReview = status.state === "locked";
        send(({ locked: "locked", app_not_running: "unavailable", unsupported: "unsupported", error: "error" })[status.state], host);
        return;
      }
      const result = await native.request("queryMatches", snapshot.pageUrl);
      await checkCurrent(current, snapshot);
      native.close();
      native = null;
      if (!result.matches.length) {
        canReview = true;
        send("no_matches", host);
        return;
      }
      const inspection = await inspectForm(api, snapshot);
      await checkCurrent(current, snapshot);
      if (!FILL_STAGES.includes(inspection.stage)) {
        const state = inspection.stage === "challenge" && inspection.kind === "website"
          ? "security_challenge"
          : ({ unsupported: "no_login_form", ambiguous: "ambiguous_login_form", challenge: "challenge",
            passwordless: "passwordless" })[inspection.stage];
        send(state, host, [], inspection.stage);
        return;
      }
      page = { ...snapshot, stage: inspection.stage, frameId: inspection.frameId };
      for (const match of result.matches) offeredIds.add(match.credentialId);
      send("matches", host, result.matches, inspection.stage);
    } catch (error) {
      if (!closed && job === current) {
        const state = errorState(error);
        canReview = ["locked", "no_matches"].includes(state) && Boolean(page);
        send(state, host);
      }
    } finally {
      native?.close();
      if (job === current) { timers.clearTimeout(current.timer); job = null; }
    }
  }
  async function requestReview(current, snapshot) {
    let native;
    try {
      await checkCurrent(current, snapshot);
      native = openNative(api, current.controller.signal, timers);
      await native.request("requestHostApproval", snapshot.pageUrl);
      native.close();
      native = null;
      await checkCurrent(current, snapshot);
      canReview = false;
      send("approval_requested", snapshot.host);
    } catch (error) {
      if (!closed && job === current) send(errorState(error), snapshot.host);
    } finally {
      native?.close();
      if (job === current) { timers.clearTimeout(current.timer); job = null; }
    }
  }
  async function fill(current, snapshot, selectedId) {
    let native;
    let payload = null;
    const discard = () => { releaseFill(payload); payload = null; };
    current.controller.signal.addEventListener("abort", discard, { once: true });
    try {
      await checkCurrent(current, snapshot);
      const inspection = await inspectForm(api, snapshot);
      if (inspection.stage !== snapshot.stage || inspection.frameId !== snapshot.frameId) {
        throw new PublicFailure("PAGE_CHANGED");
      }
      await checkCurrent(current, snapshot);
      native = openNative(api, current.controller.signal, timers);
      // The existing Rust operation rechecks auth, selected ID and stored host.
      payload = await native.request("requestFill", snapshot.pageUrl, selectedId);
      native.close();
      native = null;
      if (closed || job !== current) return;
      await checkCurrent(current, snapshot);
      if (snapshot.stage === "username_only") payload.password = "";
      if (snapshot.stage === "password_only") payload.username = "";
      await injectFill(api, snapshot, inspection.frameId, inspection.documentId, snapshot.stage, payload);
      discard();
      await checkCurrent(current, snapshot);
      send("filled", snapshot.host, [], snapshot.stage);
    } catch (error) {
      if (!closed && job === current) send(errorState(error), snapshot.host);
    } finally {
      discard();
      current.controller.signal.removeEventListener("abort", discard);
      native?.close();
      if (job === current) { timers.clearTimeout(current.timer); job = null; }
    }
  }
  function onMessage(message) {
    if (exact(message, ["version", "requestId", "type"])
      && message.version === 1 && requestId(message.requestId) && message.type === "review") {
      if (job || !page || !canReview) { dispose(); return; }
      id = message.requestId;
      const current = startJob();
      void requestReview(current, page);
      return;
    }
    if (exact(message, ["version", "requestId", "type", "credentialId"])
      && message.version === 1 && requestId(message.requestId) && message.type === "fill"
      && credentialId(message.credentialId)) {
      if (job) return; // never overlap or queue Fill requests
      if (!page || !offeredIds.has(message.credentialId)) { dispose(); return; }
      const snapshot = page;
      offeredIds.clear(); // consume the selection; another attempt needs Refresh
      id = message.requestId;
      const current = startJob();
      send("filling", snapshot.host, [], snapshot.stage);
      void fill(current, snapshot, message.credentialId);
      return;
    }
    if (!exact(message, ["version", "requestId", "type"]) || message.version !== 1
      || !requestId(message.requestId) || message.type !== "refresh") { dispose(); return; }
    cancel();
    page = null;
    canReview = false;
    offeredIds.clear();
    id = message.requestId;
    const current = startJob();
    send("loading");
    void refresh(current);
  }
  function startJob() {
    const current = { controller: new AbortController(), timer: null };
    job = current;
    current.timer = timers.setTimeout(() => { cancel(); page = null; canReview = false; offeredIds.clear(); send("unavailable"); }, 30000);
    return current;
  }
  function onUpdated(tabId, change) {
    if ((!page || page.tabId === tabId) && (change.url !== undefined || change.status === "loading")) invalidate();
  }
  function onActivated(info) { if (!page || info.windowId === page.windowId) invalidate(); }
  function onRemoved(tabId) { if (!page || page.tabId === tabId) invalidate(); }
  function onFocusChanged(windowId) { if (!page || windowId !== page.windowId) invalidate(); }
  function onDisconnect() { void api.runtime.lastError; dispose(); }
  function dispose() {
    if (closed) return;
    closed = true;
    cancel();
    page = null;
    canReview = false;
    offeredIds.clear();
    id = null;
    port.onMessage.removeListener(onMessage);
    port.onDisconnect.removeListener(onDisconnect);
    api.tabs.onUpdated.removeListener(onUpdated);
    api.tabs.onActivated.removeListener(onActivated);
    api.tabs.onRemoved.removeListener(onRemoved);
    api.windows.onFocusChanged.removeListener(onFocusChanged);
    try { port.disconnect(); } catch { /* already closed */ }
  }
  port.onMessage.addListener(onMessage);
  port.onDisconnect.addListener(onDisconnect);
  api.tabs.onUpdated.addListener(onUpdated);
  api.tabs.onActivated.addListener(onActivated);
  api.tabs.onRemoved.addListener(onRemoved);
  api.windows.onFocusChanged.addListener(onFocusChanged);
  return dispose;
}
