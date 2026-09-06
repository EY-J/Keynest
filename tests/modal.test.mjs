import assert from "node:assert/strict";
import test from "node:test";
import { mount } from "./componentHarness.mjs";

async function fixture(overrides = {}, duration = "0.18s") {
  class Element {}
  const opener = Object.assign(new Element(), { isConnected: true, focus() { focused = "opener"; } });
  let focused, shown = 0, closed = 0, requested = 0, exited = 0;
  const root = { style: { overflow: "auto" }, classList: { add() {}, remove() {} } };
  const field = { focus() { focused = "field"; }, getClientRects: () => [1], closest: () => null };
  const last = { focus() { focused = "last"; }, getClientRects: () => [1], closest: () => null };
  const dialog = { showModal() { shown++; }, close() { closed++; }, focus() { focused = "dialog"; },
    querySelector: () => field, querySelectorAll: () => [field, last],
    getBoundingClientRect: () => ({ left: 100, right: 620, top: 110, bottom: 500 }) };
  const f = await mount("../src/shared/components/Modal/Modal.tsx", {
    titleId: "fixture-title", closing: false, children: "fixture content", onClose() { requested++; }, onExitComplete() { exited++; }, ...overrides,
  }, {}, { document: { activeElement: opener, documentElement: root },
    globals: { HTMLElement: Element, getComputedStyle: () => ({ animationDuration: duration }) },
    attachRefs(tree) { tree.props.ref.current = dialog; },
  });
  return { ...f, root, dialog, field, last, shown: () => shown, closed: () => closed,
    requested: () => requested, exited: () => exited, focused: () => focused,
    pointer(x, y) { return { target: dialog, currentTarget: dialog, clientX: x, clientY: y }; },
    end(name = "keynest-modal-exit", pseudoElement = "") {
      f.tree().props.onAnimationEnd({ target: dialog, currentTarget: dialog, animationName: name, nativeEvent: { pseudoElement } });
    } };
}
test("shared native modal opens on top layer, focuses without scrolling, traps Tab and restores scroll/focus", async () => {
  const f = await fixture();
  assert.equal(f.shown(), 1); assert.equal(f.root.style.overflow, "hidden");
  assert.equal(f.tree().props["aria-labelledby"], "fixture-title");
  assert.equal(f.tree().props["aria-modal"], "true");
  f.expire(); assert.equal(f.focused(), "field");
  for (const shiftKey of [true, false]) {
    f.document.activeElement = shiftKey ? f.field : f.last;
    let prevented = false;
    f.tree().props.onKeyDown({ key: "Tab", shiftKey, currentTarget: f.dialog, preventDefault() { prevented = true; } });
    assert.ok(prevented); assert.equal(f.focused(), shiftKey ? "last" : "field");
  }
  f.unmount(); assert.equal(f.closed(), 1); assert.equal(f.focused(), "opener"); assert.equal(f.root.style.overflow, "auto");
});
test("backdrop and Escape obey each popup's rules, pending blocks both and dragging out does not dismiss", async () => {
  const f = await fixture();
  const click = (start, end) => { f.tree().props.onPointerDown(f.pointer(...start)); f.tree().props.onClick(f.pointer(...end)); };
  click([1, 1], [1, 1]); assert.equal(f.requested(), 0);
  f.props.closeOnBackdrop = true; f.render();
  click([110, 120], [1, 1]); assert.equal(f.requested(), 0);
  click([110, 120], [110, 120]); assert.equal(f.requested(), 0);
  click([1, 1], [1, 1]); assert.equal(f.requested(), 1);
  f.props.pending = true; f.render(); click([1, 1], [1, 1]);
  f.tree().props.onCancel({ preventDefault() {} }); assert.equal(f.requested(), 1);
  f.props.pending = false; f.props.closeOnEscape = false; f.render();
  f.tree().props.onCancel({ preventDefault() {} }); assert.equal(f.requested(), 1);
  f.props.closeOnEscape = true; f.render();
  f.tree().props.onCancel({ preventDefault() {} }); assert.equal(f.requested(), 2); f.unmount();
});
test("closing retains modal and scroll lock until surface exit, ignores backdrop events, reduced motion has fallback", async () => {
  for (const duration of ["0.18s", "10ms"]) {
    const f = await fixture({}, duration); f.expire();
    f.props.closing = true; f.render(); await f.flush();
    assert.equal(f.tree().props["data-state"], "closing");
    assert.equal(f.tree().props.children.props.inert, true);
    assert.equal(f.root.style.overflow, "hidden");
    assert.deepEqual([...f.timers.values()], [duration === "10ms" ? 60 : 230]);
    f.end("keynest-modal-fade-out", "::backdrop"); assert.equal(f.exited(), 0);
    f.expire(); assert.equal(f.exited(), 1);
    f.unmount(); assert.equal(f.timers.size, 0);
  }
});
test("forced security teardown cancels pending animation without running delayed actions", async () => {
  const f = await fixture(); f.props.closing = true; f.render(); await f.flush();
  f.unmount(); f.expire(); assert.equal(f.exited(), 0); assert.equal(f.root.style.overflow, "auto");
});
