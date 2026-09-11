import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import vm from "node:vm";
import { transformWithOxc } from "vite";

// Hook/IPC ports for lifecycle tests; not a substitute for browser rendering tests.
export async function mount(relative, props, dependencies = {}, environment = {}) {
  const hooks = [], effects = [], timers = new Map(), listeners = new Map();
  let cursor = 0, dirty = true, tree, mounted = true;
  const window = {
    setTimeout(fn, ms) { timers.set(fn, ms); return fn; },
    clearTimeout(fn) { timers.delete(fn); },
    addEventListener(name, fn) { if (!listeners.has(name)) listeners.set(name, new Set()); listeners.get(name).add(fn); },
    removeEventListener(name, fn) { listeners.get(name)?.delete(fn); },
  };
  const document = { hidden: false, addEventListener: window.addEventListener, removeEventListener: window.removeEventListener, ...environment.document };
  const context = vm.createContext({ window, document, queueMicrotask, ...environment.globals });
  const element = (type, props, key) => typeof type === "function" && type.name === "ModalCloseButton" ? type(props) : ({ type, props, key });
  const modules = {
    react: {
      useState(initial) {
        const i = cursor++;
        if (!(i in hooks)) hooks[i] = typeof initial === "function" ? initial() : initial;
        return [hooks[i], value => {
          if (!mounted) return;
          const next = typeof value === "function" ? value(hooks[i]) : value;
          if (!Object.is(next, hooks[i])) { hooks[i] = next; dirty = true; }
        }];
      },
      useRef(initial) { return hooks[cursor++] ??= { current: initial }; },
      useCallback(fn, deps) {
        const i = cursor++, prev = hooks[i];
        if (!prev || deps.some((v, n) => !Object.is(v, prev.deps[n]))) hooks[i] = { deps, fn };
        return hooks[i].fn;
      },
      useEffect(fn, deps) {
        const i = cursor++, prev = hooks[i];
        if (!prev || deps.some((v, n) => !Object.is(v, prev.deps[n]))) {
          effects.push(() => { prev?.cleanup?.(); hooks[i] = { deps, cleanup: fn() }; });
        }
      },
    },
    "react/jsx-runtime": {
      jsx: element, jsxs: element, Fragment: "fragment",
    },
    "lucide-react": { X: "icon" }, "./modal.css": {},
    ...dependencies,
  };
  const file = new URL(relative, import.meta.url);
  const { code } = await transformWithOxc(await readFile(file, "utf8"), file.pathname);
  const module = new vm.SourceTextModule(code, { context });
  const modalFile = new URL("../src/components/ui/Modal/Modal.tsx", import.meta.url);
  const modalCode = (await transformWithOxc(await readFile(modalFile, "utf8"), modalFile.pathname)).code;
  const modalModule = new vm.SourceTextModule(modalCode, { context });
  await module.link(specifier => {
    if (specifier.endsWith("/Modal/Modal")) return modalModule;
    assert.ok(specifier in modules, `Unexpected dependency: ${specifier}`);
    const exports = modules[specifier];
    return new vm.SyntheticModule(Object.keys(exports), function () {
      for (const [key, value] of Object.entries(exports)) this.setExport(key, value);
    }, { context });
  });
  await module.evaluate();
  function nodes(node = tree) {
    if (!node || typeof node !== "object") return [];
    return [node, ...[node.props?.children].flat(Infinity).filter(Boolean).flatMap(child => nodes(child))];
  }
  function render() {
    dirty = false; cursor = 0; tree = module.namespace.default(props);
    environment.attachRefs?.(tree);
    effects.splice(0).forEach(fn => fn());
  }
  async function flush() {
    for (let i = 0; i < 20; i++) {
      if (dirty) render();
      await new Promise(setImmediate);
      if (!dirty) return;
    }
    assert.fail("Component did not settle");
  }
  await flush();
  return { props, render, flush, nodes, timers, listeners, document,
    tree: () => tree, find: predicate => nodes().find(predicate),
    unmount() { mounted = false; hooks.forEach(hook => hook?.cleanup?.()); },
    fire(name) { listeners.get(name)?.forEach(fn => fn()); },
    expire() { const pending = [...timers.keys()]; pending.forEach(fn => { timers.delete(fn); fn(); }); },
  };
}

export function deferred() {
  let resolve, reject;
  const promise = new Promise((a, b) => { resolve = a; reject = b; });
  return { promise, resolve, reject };
}
