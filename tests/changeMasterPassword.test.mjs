import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import { transformWithOxc } from "vite";

// Exercise the component's handlers with hook/native-dialog ports, not a real vault.
// These are not browser rendering or native focus/inertness tests.
const filename = new URL("../src/features/settings/components/ChangeMasterPasswordForm.tsx", import.meta.url);
const { code } = await transformWithOxc(await readFile(filename, "utf8"), filename.pathname);
const policyFile = new URL("../src/shared/security/masterPasswordPolicy.ts", import.meta.url);
const { code: policyCode } = await transformWithOxc(await readFile(policyFile, "utf8"), policyFile.pathname);
async function compile(relative) {
  const file = new URL(relative, import.meta.url);
  return (await transformWithOxc(await readFile(file, "utf8"), file.pathname)).code;
}
const estimatorCode = await compile("../src/shared/security/passwordStrength.ts");
const indicatorCode = await compile("../src/shared/components/MasterPasswordStrength.tsx");
const setupCode = await compile("../src/features/auth/components/SetupScreen.tsx");
const recoveryCode = await compile("../src/features/auth/components/RecoverPasswordDialog.tsx");
const modalCode = await compile("../src/shared/components/Modal/Modal.tsx");
const unlockCode = await compile("../src/features/auth/components/UnlockScreen.tsx");

async function fixture(changePassword = async () => "unlocked", flow = "change") {
  const hooks = [];
  const pendingEffects = [];
  const calls = [];
  let cursor = 0;
  let tree;
  let focused;
  let showCount = 0;
  let closeCount = 0;
  const frames = new Map();
  const timers = new Map();
  const dialog = {
    showModal() { showCount++; },
    close() { closeCount++; },
    focus() { focused = "dialog"; },
    querySelector() { return input; },
    getBoundingClientRect: () => ({ left: 200, right: 720, top: 110, bottom: 540 }),
  };
  const trigger = { isConnected: true, focus() { focused = "trigger"; } };
  const input = { focus() { focused = "current"; } };
  class HTMLElement {}
  Object.setPrototypeOf(trigger, HTMLElement.prototype);
  const context = vm.createContext({
    HTMLElement,
    document: { activeElement: trigger, documentElement: { style: { overflow: "" }, classList: { add() {}, remove() {} } } },
    requestAnimationFrame(callback) { frames.set(callback, callback); return callback; },
    cancelAnimationFrame(id) { frames.delete(id); },
    getComputedStyle: () => ({ animationDuration: "0.18s" }),
    window: {
      addEventListener() {},
      removeEventListener() {},
      setTimeout(callback, duration) { timers.set(callback, duration); return callback; },
      clearTimeout(id) { timers.delete(id); },
    },
  });
  let modalHookStart = 0;
  const element = (type, props) => {
    if (typeof type !== "function") return { type, props };
    if (type.name === "Modal") modalHookStart = cursor;
    return type(props);
  };
  class AuthClientError extends Error {}
  const modules = {
    react: {
      useState(initial) {
        const index = cursor++;
        if (!(index in hooks)) hooks[index] = initial;
        return [hooks[index], value => { hooks[index] = value; }];
      },
      useRef(initial) {
        const index = cursor++;
        return hooks[index] ??= { current: initial };
      },
      useCallback(callback) {
        const index = cursor++;
        return hooks[index] ??= callback;
      },
      useEffect(effect, deps) {
        const index = cursor++;
        const previous = hooks[index];
        if (!previous || deps.some((value, i) => value !== previous.deps[i])) {
          pendingEffects.push(() => {
            previous?.cleanup?.();
            hooks[index] = { deps, cleanup: effect() };
          });
        }
      },
    },
    "react/jsx-runtime": { jsx: element, jsxs: element, Fragment: "fragment" },
    "lucide-react": { LockKeyhole: "lock", X: "x", KeyRound: "key", TriangleAlert: "warning", ChevronRight: "chevron" },
    "../../auth/authClient": { authClient: { changeMasterPassword(...args) {
      calls.push(args);
      return changePassword(...args);
    } } },
    "../../auth/components/PasswordField": { default: "PasswordField" },
    "../../auth/types": { AuthClientError },
    "./SettingsRow": { default: "SettingsRow" },
    "./ChangeMasterPasswordForm.css": {},
    "./RecoverPasswordDialog.css": {},
    "./modal.css": {},
  };
  modules["../authClient"] = { authClient: {
    unlock(...args) { calls.push(args); return changePassword(...args); },
    createMasterPassword(...args) { calls.push(args); return changePassword(...args); },
    recoverMasterPassword(...args) { calls.push(args); return changePassword(...args); },
  } };
  modules["../types"] = { AuthClientError };
  modules["./PasswordField"] = { default: "PasswordField" };
  modules["./AuthLayout"] = { default: "AuthLayout" };
  modules["./RecoveryKeyScreen"] = { default: "RecoveryKeyScreen" };
  modules["./ResetDialog"] = { default: "ResetDialog" };
  modules["./RecoverPasswordDialog"] = { default: "RecoverPasswordDialog" };
  modules["./LockScreenBackground"] = { default: "LockScreenBackground" };
  modules["./MasterPasswordStrength.css"] = {};
  const source = flow === "setup" ? setupCode : flow === "recovery" ? recoveryCode : flow === "unlock" ? unlockCode : code;
  const module = new vm.SourceTextModule(source, { context });
  const modalModule = new vm.SourceTextModule(modalCode, { context });
  const policyModule = new vm.SourceTextModule(policyCode, { context });
  const estimatorModule = new vm.SourceTextModule(estimatorCode, { context });
  const indicatorModule = new vm.SourceTextModule(indicatorCode, { context });
  await module.link(specifier => {
    if (specifier.endsWith("/Modal/Modal")) return modalModule;
    if (specifier.endsWith("/masterPasswordPolicy")) return policyModule;
    if (specifier === "./passwordStrength") return estimatorModule;
    if (specifier.endsWith("/MasterPasswordStrength")) return indicatorModule;
    assert.ok(specifier in modules, `Unexpected dependency: ${specifier}`);
    const exports = modules[specifier];
    return new vm.SyntheticModule(Object.keys(exports), function () {
      for (const [key, value] of Object.entries(exports)) this.setExport(key, value);
    }, { context });
  });
  await module.evaluate();
  function nodes(node = tree) {
    if (!node || typeof node !== "object") return [];
    return [node, ...[node.props.children].flat().filter(Boolean).flatMap(child => nodes(child))];
  }
  const find = predicate => nodes().find(predicate);
  const recovered = [];
  const props = { isOpen: true, onClose() {}, onRecovered(result) { recovered.push(result); }, onChooseReset() {}, onCreated() {}, onUnlocked() { recovered.push("unlocked"); } };
  let previousDialog = false;
  function render() {
    const beforeEffects = new Set(hooks.slice(modalHookStart).filter(h => h?.cleanup));
    cursor = 0;
    tree = module.namespace.default(props);
    for (const node of nodes()) {
      if (node.props.ref) node.props.ref.current = node.type === "dialog" ? dialog : trigger;
      if (node.props.inputRef) node.props.inputRef.current = input;
    }
    const hasDialog = nodes().some(n => n.type === "dialog");
    if (previousDialog && !hasDialog) {
      for (const hook of beforeEffects) { hook.cleanup?.(); hook.cleanup = undefined; }
      hooks.splice(cursor);
    }
    previousDialog = hasDialog;
    pendingEffects.splice(0).forEach(effect => effect());
  }
  render();
  for (const [timer, duration] of timers) if (duration === 0) { timers.delete(timer); timer(); }
  function open() { find(node => node.props.children === "Change").props.onClick(); render(); }
  function fill(current = "old-password", next = "new-password-123", confirmation = next) {
    const fields = nodes().filter(node => node.type === "PasswordField");
    [current, next, confirmation].forEach((value, i) => fields[i].props.onChange(value));
    render();
  }
  const dismiss = () => find(node => node.type === "dialog").props.onCancel({ preventDefault() {} });
  const finish = (animationName = "keynest-modal-exit", pseudoElement = "") => {
    find(node => node.type === "dialog").props.onAnimationEnd({
      target: dialog, currentTarget: dialog, animationName, nativeEvent: { pseudoElement },
    });
    render();
  };
  const submit = async () => {
    find(node => node.type === "form").props.onSubmit({ preventDefault() {} });
    await new Promise(setImmediate);
    render();
  };
  return { open, fill, dismiss, finish, submit, render, find, nodes, calls, frames, timers, recovered, props,
    AuthClientError, dialog, context, focused: () => focused,
    showCount: () => showCount, closeCount: () => closeCount };
}

for (const flow of ["unlock", "recovery"]) {
  test(`${flow}: backend cooldown clears secrets, blocks retries, and expires`, async () => {
    let rejectWith;
    const f = await fixture(() => {
      if (rejectWith) throw rejectWith;
      return flow === "unlock" ? "unlocked" : { status: "unlocked", recoveryKey: "replacement-key" };
    }, flow);
    if (flow === "recovery") {
      f.find(n => n.props.className === "recovery-option recovery-option--primary").props.onClick(); f.render();
    }
    const fill = () => {
      const values = flow === "unlock" ? ["a secure master password"] :
        ["fixture recovery key", "a replacement master password", "a replacement master password"];
      f.nodes().filter(n => n.type === "PasswordField").forEach((field, i) => field.props.onChange(values[i]));
      f.render();
    };
    rejectWith = Object.assign(new f.AuthClientError("Wait a moment before trying again."), { retryAfterMs: 2000 });
    fill(); await f.submit();
    assert.ok(f.nodes().filter(n => n.type === "PasswordField").every(n => n.props.value === ""));
    assert.equal(f.find(n => n.props.children === "Please wait…").props.disabled, true);
    assert.equal(f.timers.size, 1);
    assert.equal([...f.timers.values()][0], 2000);
    fill(); await f.submit();
    assert.equal(f.calls.length, 1);
    if (flow === "recovery") {
      f.props.isOpen = false; f.render();
      f.props.isOpen = true; f.render();
      for (const [timer, duration] of f.timers) if (duration === 0) { f.timers.delete(timer); timer(); }
      f.find(n => n.props.className === "recovery-option recovery-option--primary").props.onClick(); f.render();
      assert.equal(f.find(n => n.props.children === "Please wait…").props.disabled, true);
    }
    [...f.timers.keys()][0](); f.render();
    assert.equal(f.timers.size, 0);
    rejectWith = null;
    fill(); await f.submit();
    assert.equal(f.calls.length, 2);
    if (flow === "recovery") f.finish();
    assert.equal(f.recovered.length, 1);
  });

  test(`${flow}: excessively large cooldown metadata is capped`, async () => {
    const f = await fixture(() => { throw Object.assign(new f.AuthClientError("Please wait"), { retryAfterMs: Number.MAX_VALUE }); }, flow);
    if (flow === "recovery") {
      f.find(n => n.props.className === "recovery-option recovery-option--primary").props.onClick(); f.render();
    }
    const values = flow === "unlock" ? ["a secure master password"] :
      ["fixture key", "a replacement master password", "a replacement master password"];
    f.nodes().filter(n => n.type === "PasswordField").forEach((field, i) => field.props.onChange(values[i]));
    f.render(); await f.submit();
    assert.equal([...f.timers.values()][0], 30_000);
  });
}

test("opens natively, delays input focus, animates exit, clears fields and restores focus", async () => {
  const f = await fixture();
  assert.equal(f.find(n => n.type === "form"), undefined);
  f.open();
  assert.equal(f.showCount(), 1);
  for (const [timer, duration] of f.timers) if (duration === 0) { f.timers.delete(timer); timer(); }
  assert.equal(f.focused(), "current");
  f.fill(); f.dismiss(); f.render();
  assert.equal(f.find(n => n.type === "dialog").props["data-state"], "closing");
  f.finish("keynest-modal-fade-out", "::backdrop");
  assert.ok(f.find(n => n.type === "dialog"));
  f.finish();
  assert.equal(f.find(n => n.type === "dialog"), undefined);
  assert.equal(f.closeCount(), 1);
  assert.equal(f.focused(), "trigger");
  f.open();
  assert.ok(f.nodes().filter(n => n.type === "PasswordField").every(n => n.props.value === ""));
});

test("validation messages and Unicode minimum length stay unchanged", async () => {
  const f = await fixture(); f.open();
  for (const [values, message] of [
    [["", "valid-password", "valid-password"], "Enter your current Master Password."],
    [["old", "😀".repeat(11), "😀".repeat(11)], "Use at least 12 characters."],
    [["old", "valid-password", "different"], "The passwords do not match."],
  ]) {
    f.fill(...values); await f.submit();
    assert.equal(f.find(n => n.props.role === "alert").props.children, message);
  }
  assert.equal(f.calls.length, 0);
});

test("pending submission blocks dismissals and duplicate calls; success closes with feedback", async () => {
  let resolve;
  const f = await fixture(() => new Promise(done => { resolve = done; }));
  f.open(); f.fill(); await f.submit();
  f.dismiss(); f.render(); await f.submit();
  assert.equal(f.find(n => n.type === "dialog").props["data-state"], "open");
  assert.equal(f.calls.length, 1);
  assert.deepEqual(f.calls[0], ["old-password", "new-password-123"]);
  assert.ok(f.nodes().filter(n => n.type === "PasswordField").every(n => n.props.disabled));
  resolve("unlocked"); await new Promise(setImmediate); f.render();
  assert.equal(f.find(n => n.type === "dialog").props["data-state"], "closing");
  f.finish();
  assert.match(f.find(n => n.props.role === "status").props.children, /^Master Password changed\./);
});

test("backend errors remain inside the dialog and clear submitted passwords", async () => {
  const f = await fixture(() => { throw new f.AuthClientError("Incorrect current password"); });
  f.open(); f.fill(); await f.submit();
  assert.equal(f.find(n => n.props.role === "alert").props.children, "Incorrect current password");
  assert.equal(f.find(n => n.type === "dialog").props["data-state"], "open");
  assert.ok(f.nodes().filter(n => n.type === "PasswordField").every(n => n.props.value === ""));
});

test("reduced-motion exit finishes and Tab wraps in both directions", async () => {
  const f = await fixture(); f.open();
  let focused;
  const first = { getClientRects: () => [1], closest: () => null, focus() { focused = "first"; } };
  const last = { getClientRects: () => [1], closest: () => null, focus() { focused = "last"; } };
  const target = { querySelectorAll: () => [first, last] };
  let prevented = 0;
  for (const shiftKey of [false, true]) {
    f.context.document.activeElement = shiftKey ? first : last;
    f.find(n => n.type === "dialog").props.onKeyDown({
      key: "Tab", shiftKey, currentTarget: target, preventDefault() { prevented++; },
    });
    assert.equal(focused, shiftKey ? "last" : "first");
  }
  assert.equal(prevented, 2);
  f.dismiss(); f.render(); f.finish("keynest-modal-fade-out");
  assert.equal(f.find(n => n.type === "dialog"), undefined);
});

test("Cancel, X and backdrop dismiss; dialog padding and dragging out do not", async () => {
  for (const method of ["cancel", "close", "backdrop"]) {
    const f = await fixture(); f.open();
    const props = f.find(n => n.type === "dialog").props;
    const pointer = (x, y) => ({ target: f.dialog, currentTarget: f.dialog, clientX: x, clientY: y });
    props.onPointerDown(pointer(210, 120)); props.onClick(pointer(210, 120)); f.render();
    assert.equal(f.find(n => n.type === "dialog").props["data-state"], "open");
    props.onPointerDown(pointer(210, 120)); props.onClick(pointer(10, 60)); f.render();
    assert.equal(f.find(n => n.type === "dialog").props["data-state"], "open");
    if (method === "cancel") f.find(n => n.props.children === "Cancel").props.onClick();
    else if (method === "close") f.find(n => n.props["aria-label"] === "Close Change Master Password").props.onClick();
    else { props.onPointerDown(pointer(10, 60)); props.onClick(pointer(10, 60)); }
    f.render();
    assert.equal(f.find(n => n.type === "dialog").props["data-state"], "closing");
  }
});

test("unconfirmed backend status does not report success or dismiss", async () => {
  const f = await fixture(async () => "locked");
  f.open(); f.fill(); await f.submit();
  assert.match(f.find(n => n.props.role === "alert").props.children, /could not confirm/);
  assert.equal(f.find(n => n.type === "dialog").props["data-state"], "open");
  assert.equal(f.find(n => n.props.className === "security-success"), undefined);
});

test("strength visibility, submission eligibility and mismatch feedback", async () => {
  const f = await fixture(); f.open();
  const submitButton = () => f.find(n => n.props.className === "primary-button compact-button");
  const meter = () => f.find(n => n.props.className === "master-password-strength");
  assert.equal(meter(), undefined);
  assert.equal(submitButton().props.disabled, true);
  for (const [password, expected] of [["123456789012", "Weak"], ["aaaaaaaaaaaa", "Good"], ["V7!qR2@tL9#z", "Strong"]]) {
    f.fill("old", password);
    assert.equal(meter().props["data-strength"], expected);
    assert.equal(submitButton().props.disabled, expected === "Weak");
  }
  f.fill("old", "V7!qR2@tL9#z", "mismatch");
  assert.equal(submitButton().props.disabled, true);
  assert.equal(f.find(n => n.props.className === "master-password-modal__mismatch").props.children,
    "Passwords do not match.");
  f.fill("old", "V7!qR2@tL9#z", "");
  assert.equal(submitButton().props.disabled, true);
  assert.equal(f.find(n => n.props.className === "master-password-modal__mismatch"), undefined);
  f.fill("", "V7!qR2@tL9#z");
  assert.equal(submitButton().props.disabled, true);
  f.fill("old", "123456789012"); await f.submit();
  assert.equal(f.calls.length, 0);
  assert.equal(f.find(n => n.props.role === "alert").props.children, "Master Password is too weak.");
});

test("shared password-policy vectors match the reused Vault estimate", async () => {
  const policy = new vm.SourceTextModule(policyCode);
  await policy.link(specifier => {
    assert.equal(specifier, "./passwordStrength");
    return new vm.SourceTextModule(estimatorCode);
  });
  await policy.evaluate();
  const cases = JSON.parse(await readFile(new URL("./fixtures/master-password-policy.json", import.meta.url), "utf8"));
  for (const entry of cases) {
    assert.equal(policy.namespace.masterPasswordStrength(entry.password), entry.strength, entry.id);
  }
});

for (const flow of ["setup", "recovery"]) {
  test(`${flow}: shared policy blocks weak, short, empty and mismatched passwords`, async () => {
    const f = await fixture(undefined, flow);
    if (flow === "recovery") {
      f.find(n => n.props.className === "recovery-option recovery-option--primary").props.onClick(); f.render();
    }
    const button = () => f.find(n => n.props.children === (flow === "setup" ? "Create Master Password" : "Recover KeyNest"));
    const meter = () => f.find(n => n.props.className === "master-password-strength");
    const fill = (password, confirmation = password) => {
      const fields = f.nodes().filter(n => n.type === "PasswordField");
      const values = flow === "setup" ? [password, confirmation] : ["test-recovery-key", password, confirmation];
      values.forEach((value, i) => fields[i].props.onChange(value)); f.render();
    };
    assert.equal(meter(), undefined);
    assert.equal(button().props.disabled, true);
    for (const [password, confirmation] of [["", ""], ["Ab1!", "Ab1!"], ["123456789012", "123456789012"],
      ["V7!qR2@tL9#z", ""], ["V7!qR2@tL9#z", "different"]]) {
      fill(password, confirmation);
      assert.equal(button().props.disabled, true);
      await f.submit(); // Even a programmatic submit bypassing the button is rejected.
      assert.equal(f.calls.length, 0);
    }
    assert.equal(f.find(n => n.props.role === "alert").props.children, "The passwords do not match.");
    for (const [password, strength] of [["aaaaaaaaaaaa", "Good"], ["V7!qR2@tL9#z", "Strong"]]) {
      fill(password);
      assert.equal(button().props.disabled, false);
      assert.equal(meter().props["data-strength"], strength);
    }
  });

  test(`${flow}: accepted request stays disabled while pending and clears fields on completion`, async () => {
    let resolve;
    const f = await fixture(() => new Promise(done => { resolve = done; }), flow);
    if (flow === "recovery") {
      f.find(n => n.props.className === "recovery-option recovery-option--primary").props.onClick(); f.render();
    }
    const fields = f.nodes().filter(n => n.type === "PasswordField");
    const password = "V7!qR2@tL9#z";
    const values = flow === "setup" ? [password, password] : ["test-recovery-key", password, password];
    values.forEach((value, i) => fields[i].props.onChange(value)); f.render();
    await f.submit();
    assert.ok(f.nodes().filter(n => n.type === "PasswordField").every(n => n.props.disabled));
    await f.submit();
    assert.equal(f.calls.length, 1);
    const result = { status: "unlocked", recoveryKey: "test-replacement-key" };
    resolve(result); await new Promise(setImmediate); f.render();
    if (flow === "setup") assert.ok(f.find(n => n.type === "RecoveryKeyScreen"));
    else {
      assert.equal(f.recovered.length, 0, "recovery display waits for modal exit");
      f.finish();
      assert.equal(f.recovered.length, 1);
      assert.ok(f.nodes().filter(n => n.type === "PasswordField").every(n => n.props.value === ""));
    }
  });
}
