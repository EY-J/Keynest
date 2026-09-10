// Minimal event/DOM ports for deterministic lifecycle tests, not browser acceptance.
export class Event {
  listeners = new Set();
  addListener = fn => this.listeners.add(fn);
  removeListener = fn => this.listeners.delete(fn);
  emit(...args) { for (const fn of [...this.listeners]) fn(...args); }
}

export function port(sender) {
  return {
    name: "keynest-autofill-popup-v1", sender,
    onMessage: new Event(), onDisconnect: new Event(), sent: [], disconnected: false,
    postMessage(message) { this.sent.push(structuredClone(message)); this.respond?.(message); },
    disconnect() { this.disconnected = true; },
  };
}

export function timers() {
  let next = 0;
  const pending = new Map();
  return {
    pending,
    setTimeout(fn, ms) { const id = ++next; pending.set(id, { fn, ms }); return id; },
    clearTimeout(id) { pending.delete(id); },
    expire(ms) { for (const [id, task] of [...pending]) if (task.ms === ms) { pending.delete(id); task.fn(); } },
  };
}

export const matches = [{ credentialId: "a".repeat(32), name: "Fixture account", username: "user@example.test" }];
export function reply(request, type, data) {
  return { version: 1, requestId: request.requestId, ok: true, type, data };
}
export function failure(request, code) {
  return { version: 1, requestId: request.requestId, ok: false, type: "error", error: { code, message: "sentinel private path" } };
}

export function browser() {
  const native = [];
  const api = {
    current: { id: 7, windowId: 9, url: "https://fixture.test/login", status: "complete" },
    queryCount: 0,
    answer: request => request.type === "status" ? reply(request, "status", { state: "unlocked" }) : reply(request, "matches", { matches }),
    runtime: {
      id: "a".repeat(32), onConnect: new Event(), lastError: undefined,
      getURL: path => `chrome-extension://${"a".repeat(32)}/${path}`,
      connectNative(host) {
        const p = port(); p.host = host; native.push(p);
        p.respond = request => queueMicrotask(() => {
          const value = api.answer(request);
          if (value !== undefined) p.onMessage.emit(value);
        });
        return p;
      },
    },
    tabs: {
      async query() { api.queryCount++; return [structuredClone(api.current)]; },
      onUpdated: new Event(), onActivated: new Event(), onRemoved: new Event(),
    },
    windows: { onFocusChanged: new Event() },
    scripting: {
      async executeScript() { return [{ frameId: 0, documentId: "fixture-document-1", result: { ok: true, stage: "combined" } }]; },
    },
  };
  return { api, native, popup: () => port({ id: api.runtime.id, url: api.runtime.getURL("src/popup.html") }) };
}

class Node {
  constructor(tag) { this.tagName = tag; this.children = []; this.attributes = {}; this.textContent = ""; this.events = new Map(); }
  setAttribute(key, value) { this.attributes[key] = value; }
  append(...nodes) { this.children.push(...nodes); }
  replaceChildren(...nodes) { this.children = nodes; this.textContent = ""; }
  addEventListener(name, fn) { if (!this.events.has(name)) this.events.set(name, new Set()); this.events.get(name).add(fn); }
  removeEventListener(name, fn) { this.events.get(name)?.delete(fn); }
  fire(name, event = { isTrusted: true }) {
    this.events.get(name)?.forEach(fn => fn(event));
    if (name === "click" && typeof this.onclick === "function") this.onclick(event);
  }
}

export function dom() {
  const nodes = Object.fromEntries(["content", "status", "host", "host-value", "matches", "hint", "retry", "review"].map(id => [id, new Node(id)]));
  const document = Object.assign(new Node("document"), { hidden: false, getElementById: id => nodes[id], createElement: tag => new Node(tag) });
  return { document, window: new Node("window"), nodes };
}

export async function flush() { for (let i = 0; i < 8; i++) await new Promise(setImmediate); }
