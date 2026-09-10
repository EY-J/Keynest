import { POPUP_PORT, validView } from "./protocol.js";
import { renderView } from "./popup-view.js";

export function mountPopup(api, document, window, timers = globalThis) {
  let port = null;
  let id = null;
  let deadline = null;
  let closed = false;
  let busy = false;
  let stage = "";
  const retry = document.getElementById("retry");
  const show = (state, nextStage = "") => renderView(document, { state, stage: nextStage, host: "", matches: [] });
  function disconnect() {
    timers.clearTimeout(deadline);
    deadline = null;
    if (!port) return;
    port.onMessage.removeListener(onMessage);
    port.onDisconnect.removeListener(onDisconnect);
    try { port.disconnect(); } catch { /* already closed */ }
    port = null;
  }
  function fail() { busy = false; id = null; disconnect(); if (!closed) show("unavailable"); }
  function onDisconnect() { void api.runtime.lastError; fail(); }
  function onMessage(message) {
    if (!validView(message)) { fail(); return; }
    if (message.requestId !== id || closed) return;
    stage = message.stage;
    renderView(document, message, fill, review);
    if (!["loading", "filling"].includes(message.state)) { busy = false; timers.clearTimeout(deadline); deadline = null; }
  }
  function fill(selectedId) {
    if (closed || busy || !port) return;
    busy = true;
    show("filling", stage);
    id = crypto.randomUUID();
    deadline = timers.setTimeout(fail, 32000);
    try { port.postMessage({ version: 1, requestId: id, type: "fill", credentialId: selectedId }); }
    catch { fail(); }
  }
  function review() {
    if (closed || busy || !port) return;
    busy = true;
    id = crypto.randomUUID();
    deadline = timers.setTimeout(fail, 32000);
    try { port.postMessage({ version: 1, requestId: id, type: "review" }); }
    catch { fail(); }
  }
  function refresh() {
    if (closed || busy) return;
    busy = true;
    stage = "";
    show("loading");
    id = crypto.randomUUID();
    try {
      if (!port) {
        port = api.runtime.connect({ name: POPUP_PORT });
        port.onMessage.addListener(onMessage);
        port.onDisconnect.addListener(onDisconnect);
      }
      deadline = timers.setTimeout(fail, 32000);
      port.postMessage({ version: 1, requestId: id, type: "refresh" });
    } catch { fail(); }
  }
  function dispose() {
    if (closed) return;
    closed = true;
    id = null;
    disconnect();
    retry.removeEventListener("click", refresh);
    window.removeEventListener("pagehide", dispose);
    document.removeEventListener("visibilitychange", onVisibility);
    renderView(document, { state: "page_changed", stage: "", host: "", matches: [] });
  }
  function onVisibility() { if (document.hidden) dispose(); }
  retry.addEventListener("click", refresh);
  window.addEventListener("pagehide", dispose);
  document.addEventListener("visibilitychange", onVisibility);
  refresh();
  return dispose;
}
