import assert from "node:assert/strict";
import test from "node:test";
import { mount } from "./componentHarness.mjs";

const request = { approvalId: "approval-1", requestedHost: "accounts.google.com" };
const candidate = {
  credentialId: "a".repeat(32),
  name: "Gmail",
  username: "user@gmail.com",
  website: "https://gmail.com/",
};

async function fixture() {
  const calls = [];
  let eventHandler;
  const client = {
    pending: async () => request,
    candidates: async approvalId => { calls.push(["candidates", approvalId]); return [candidate]; },
    cancel: async approvalId => { calls.push(["cancel", approvalId]); },
    approve: async (approvalId, credentialId) => {
      calls.push(["approve", approvalId, credentialId]);
      return { credentialName: "Gmail", requestedHost: "accounts.google.com", changed: true };
    },
  };
  const component = await mount(
    "../src/features/autofill/HostApprovalModal.tsx",
    {},
    {
      "@tauri-apps/api/event": {
        listen: async (_name, handler) => { eventHandler = handler; return () => {}; },
      },
      "./hostApprovalClient": {
        hostApprovalClient: client,
        HostApprovalClientError: class extends Error {},
      },
      "../../shared/components/ServiceIcon": { default: "ServiceIcon" },
    },
  );
  return { ...component, calls, event: () => eventHandler?.() };
}

test("approval notifications are subscribed before the initial pending-state read", async () => {
  const order = [];
  const component = await mount(
    "../src/features/autofill/HostApprovalModal.tsx",
    {},
    {
      "@tauri-apps/api/event": {
        listen: async () => { order.push("listen"); return () => {}; },
      },
      "./hostApprovalClient": {
        hostApprovalClient: {
          pending: async () => { order.push("pending"); return null; },
          candidates: async () => [],
          cancel: async () => {},
          approve: async () => { throw new Error("not used"); },
        },
        HostApprovalClientError: class extends Error {},
      },
      "../../shared/components/ServiceIcon": { default: "ServiceIcon" },
    },
  );
  assert.deepEqual(order.slice(0, 2), ["listen", "pending"]);
  component.fire("focus");
  await component.flush();
  assert.deepEqual(order, ["listen", "pending", "pending"]);
  component.unmount();
});

function modal(component) {
  return component.find(node => typeof node.type === "function" && node.type.name === "Modal");
}

test("approval modal lists password-free summaries and requires an explicit credential selection", async () => {
  const f = await fixture();
  assert.deepEqual(f.calls, [["candidates", "approval-1"]]);
  assert.ok(f.nodes().some(node => node.props?.children === "accounts.google.com"));
  assert.ok(f.nodes().some(node => node.props?.children === "Gmail"));
  assert.ok(f.nodes().some(node => node.props?.children === "user@gmail.com"));
  assert.ok(f.nodes().some(node => Array.isArray(node.props?.children)
    && node.props.children.flat(Infinity).join("") === "Saved website: https://gmail.com/"));
  assert.ok(!JSON.stringify(f.tree()).toLowerCase().includes("password"));
  assert.equal(f.find(node => node.props?.children === "Allow Host"), undefined);

  f.find(node => node.props?.className === "host-approval-candidate").props.onClick();
  await f.flush();
  assert.ok(f.nodes().some(node => node.type === "dt" && node.props.children === "Saved website"));
  assert.ok(f.nodes().some(node => node.type === "dt" && node.props.children === "Requested login host"));
  assert.ok(f.find(node => node.props?.children === "Allow Host"));
  f.unmount();
});

test("Cancel, close X, and Escape all clear pending approval without approving", async () => {
  for (const dismiss of [
    f => f.find(node => node.props?.children === "Cancel").props.onClick(),
    f => f.find(node => node.props?.["aria-label"] === "Cancel host approval").props.onClick(),
    f => modal(f).props.onClose(),
  ]) {
    const f = await fixture();
    dismiss(f);
    await f.flush();
    assert.deepEqual(f.calls.filter(call => call[0] !== "candidates"), [["cancel", "approval-1"]]);
    assert.equal(f.calls.some(call => call[0] === "approve"), false);
    f.unmount();
  }
});

test("Allow Host sends only the selected nonsecret ID and waits for Rust success", async () => {
  const f = await fixture();
  f.find(node => node.props?.className === "host-approval-candidate").props.onClick();
  await f.flush();
  f.find(node => node.props?.children === "Allow Host").props.onClick();
  await f.flush();
  assert.deepEqual(f.calls.at(-1), ["approve", "approval-1", "a".repeat(32)]);
  assert.equal(f.calls.some(call => call[0] === "cancel"), false);
  assert.ok(!JSON.stringify(f.calls).toLowerCase().includes("password"));
  f.unmount();
});
