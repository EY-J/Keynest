import { HOST_NAME, PublicFailure, credentialId, eligiblePage, releaseFill, validateNativeResponse } from "./protocol.js";

// One transient connection per explicit popup refresh, at most one pending
// request. There is no reconnect loop, startup connection, or response cache.
export function openNative(api, signal, timers = globalThis) {
  let port;
  try { port = api.runtime.connectNative(HOST_NAME); }
  catch { throw new PublicFailure("NATIVE_HOST_UNAVAILABLE"); }
  let pending = null;
  let closed = false;

  function settle(error, data) {
    if (!pending) return;
    const current = pending;
    pending = null;
    timers.clearTimeout(current.timer);
    if (error) current.reject(error);
    else current.resolve(data);
  }
  function close(code = "IPC_UNAVAILABLE") {
    if (closed) return;
    closed = true;
    settle(new PublicFailure(code));
    port.onMessage.removeListener(onMessage);
    port.onDisconnect.removeListener(onDisconnect);
    signal.removeEventListener("abort", onAbort);
    try { port.disconnect(); } catch { /* already closed */ }
  }
  function onMessage(value) {
    if (!pending) { close(); return; }
    try { settle(null, validateNativeResponse(value, pending.request)); }
    catch (error) {
      settle(error instanceof PublicFailure ? error : new PublicFailure("IPC_UNAVAILABLE"));
      close();
    }
    finally { if (value?.type === "fillPayload") releaseFill(value.data); }
  }
  function onDisconnect() {
    // Reading lastError consumes Chromium diagnostics; its text is never used.
    void api.runtime.lastError;
    close("NATIVE_HOST_UNAVAILABLE");
  }
  function onAbort() { close("PAGE_CHANGED"); }
  port.onMessage.addListener(onMessage);
  port.onDisconnect.addListener(onDisconnect);
  signal.addEventListener("abort", onAbort, { once: true });
  if (signal.aborted) onAbort();

  return {
    close,
    request(type, pageUrl, selectedId) {
      if (closed || pending) return Promise.reject(new PublicFailure("IPC_UNAVAILABLE"));
      if (!["status", "queryMatches", "requestFill", "requestHostApproval"].includes(type)
        || (type === "requestFill" && !credentialId(selectedId))) return Promise.reject(new PublicFailure("INVALID_REQUEST"));
      if (type !== "status") {
        try { eligiblePage(pageUrl); } catch (error) { return Promise.reject(error); }
      }
      const request = { version: 1, requestId: crypto.randomUUID(), type };
      if (type !== "status") request.pageUrl = pageUrl;
      if (type === "requestFill") request.credentialId = selectedId;
      return new Promise((resolve, reject) => {
        pending = { request, resolve, reject, timer: timers.setTimeout(() => close(), 12000) };
        try { port.postMessage(request); } catch { close("NATIVE_HOST_UNAVAILABLE"); }
      });
    },
  };
}
