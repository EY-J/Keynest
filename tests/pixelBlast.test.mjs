import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import { transformWithOxc } from "vite";

// Exercise the actual effect with fake browser/GPU ports, without an auth profile.
const filename = new URL("../src/components/effects/PixelBlast/PixelBlast.jsx", import.meta.url);
const { code } = await transformWithOxc(await readFile(filename, "utf8"), filename.pathname);

async function fixture({ failRenderer = false, failRender = false, props = {} } = {}) {
  const resources = [];
  const frames = new Map();
  const listeners = new Map();
  const canvases = [];
  let effect;
  let frameId = 0;
  let renderCount = 0;
  class Resource {
    disposed = false;
    constructor() { resources.push(this); }
    dispose() { this.disposed = true; }
  }
  class Vector2 {
    constructor(x = 0, y = 0) { this.set(x, y); }
    set(x, y) { this.x = x; this.y = y; }
  }
  const container = {
    clientWidth: 1000, clientHeight: 660,
    appendChild(canvas) { canvases.push(canvas); }
  };
  const document = {
    hidden: false,
    createElement() {
      const events = new Map();
      return {
        events,
        getContext: () => ({ fillRect() {}, beginPath() {}, arc() {}, fill() {} }),
        getBoundingClientRect: () => ({ left: 0, top: 40, width: 1000, height: 660 }),
        addEventListener: (event, callback) => events.set(event, callback),
        removeEventListener: event => events.delete(event),
        remove() { canvases.splice(canvases.indexOf(this), 1); }
      };
    },
    addEventListener: (event, callback) => listeners.set(event, callback),
    removeEventListener: event => listeners.delete(event)
  };
  const render = () => {
    renderCount++;
    if (failRender) throw new Error("GPU unavailable");
  };
  const three = {
    WebGLRenderer: class extends Resource {
      constructor({ canvas }) {
        if (failRenderer) throw new Error("No WebGL");
        super(); this.domElement = canvas; this.debug = {};
      }
      setPixelRatio(value) { this.ratio = value; }
      getPixelRatio() { return this.ratio; }
      setClearColor() {}
      setSize(w, h) { this.domElement.width = w * this.ratio; this.domElement.height = h * this.ratio; }
      forceContextLoss() {}
      render = render;
    },
    Vector2,
    Color: class {},
    Uniform: class { constructor(value) { this.value = value; } },
    Texture: Resource, ShaderMaterial: Resource, PlaneGeometry: Resource,
    Scene: class { add() {} }, Mesh: class {}, OrthographicCamera: class {},
    LinearFilter: 1, GLSL3: 3
  };
  const postprocessing = {
    Effect: class extends Resource {
      constructor(_name, _shader, { uniforms }) { super(); this.uniforms = uniforms; }
    },
    EffectComposer: class extends Resource {
      passes = [];
      addPass(pass) { this.passes.push(pass); }
      setSize() {}
      render = render;
      dispose() { super.dispose(); this.passes.forEach(pass => pass.dispose?.()); }
    },
    RenderPass: class {},
    EffectPass: class {
      constructor(_camera, ...effects) { this.effects = effects; }
      dispose() { this.effects.forEach(effect => effect.dispose()); }
    }
  };
  class Observer extends Resource {
    observe() {}
    disconnect() { this.dispose(); }
  }
  const context = vm.createContext({
    document, window: { devicePixelRatio: 2 }, ResizeObserver: Observer,
    IntersectionObserver: Observer,
    requestAnimationFrame: callback => { frames.set(++frameId, callback); return frameId; },
    cancelAnimationFrame: id => frames.delete(id)
  });
  const modules = {
    react: { useRef: () => ({ current: container }), useEffect: callback => { effect = callback; } },
    "react/jsx-runtime": { jsx: () => null },
    three, postprocessing, "./PixelBlast.css": {}
  };
  const module = new vm.SourceTextModule(code, { context });
  await module.link(specifier => {
    assert.ok(specifier in modules, `Unexpected dependency: ${specifier}`);
    const exports = modules[specifier];
    return new vm.SyntheticModule(Object.keys(exports), function () {
      for (const [key, value] of Object.entries(exports)) this.setExport(key, value);
    }, { context });
  });
  await module.evaluate();
  module.namespace.default({ liquid: true, noiseAmount: 0.1, ...props });
  const mount = () => effect();
  const tick = () => {
    const [id, callback] = frames.entries().next().value;
    frames.delete(id); callback(1000);
  };
  const assertClean = () => {
    assert.equal(frames.size, 0);
    assert.equal(canvases.length, 0);
    assert.equal(listeners.size, 0);
    assert.ok(resources.every(resource => resource.disposed));
  };
  return { mount, tick, assertClean, frames, document, listeners, canvases, renderCount: () => renderCount };
}

test("StrictMode setup/cleanup cycles release all GPU resources and listeners", async () => {
  const f = await fixture();
  for (let cycle = 0; cycle < 2; cycle++) {
    const cleanup = f.mount();
    const canvas = f.canvases[0];
    f.tick();
    assert.equal(f.frames.size, 1);
    cleanup(); cleanup();
    assert.equal(canvas.events.size, 0);
    f.assertClean();
  }
});

test("hidden documents cancel frames and resume with one animation loop", async () => {
  const f = await fixture();
  const cleanup = f.mount();
  f.document.hidden = true;
  f.listeners.get("visibilitychange")();
  assert.equal(f.frames.size, 0);
  f.document.hidden = false;
  f.listeners.get("visibilitychange")();
  f.listeners.get("visibilitychange")();
  assert.equal(f.frames.size, 1);
  f.tick();
  assert.equal(f.renderCount(), 1);
  cleanup(); f.assertClean();
});

test("unavailable WebGL and render failures leave no background resources", async () => {
  for (const options of [{ failRenderer: true }, { failRender: true }]) {
    const f = await fixture(options);
    const cleanup = f.mount();
    if (f.frames.size) f.tick();
    cleanup(); f.assertClean();
  }
});

test("context loss cancels rendering and releases the canvas", async () => {
  const f = await fixture();
  const cleanup = f.mount();
  f.canvases[0].events.get("webglcontextlost")({ preventDefault() {} });
  f.assertClean(); cleanup();
});

test("ambient background animates without any mouse or click listeners", async () => {
  const f = await fixture({ props: { liquid: false, enableRipples: false, noiseAmount: 0 } });
  const cleanup = f.mount();
  const events = f.canvases[0].events;
  assert.equal(events.has("pointermove"), false);
  assert.equal(events.has("pointerdown"), false);
  f.tick();
  assert.equal(f.renderCount(), 1);
  cleanup(); f.assertClean();
});
