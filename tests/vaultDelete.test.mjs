import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { mount } from "./componentHarness.mjs";
import { summary, vaultFixture } from "./vaultFixture.mjs";

async function deleteDialog({ deleteCredential, onDeleted, onCancel } = {}) {
  const calls = [];
  const toasts = [];
  let cancelled = 0;
  let refreshed = 0;
  const credential = summary("a");
  const fixture = await mount("../src/features/vault/components/DeleteCredentialDialog.tsx", {
    credential,
    onCancel: () => {
      cancelled += 1;
      onCancel?.();
    },
    onDeleted: async () => {
      refreshed += 1;
      await onDeleted?.();
    },
  }, {
    "../vaultClient": {
      vaultClient: {
        deleteCredential: async id => {
          calls.push(["delete", id]);
          await deleteCredential?.(id);
        },
      },
    },
    "./VaultModal": { default: "VaultModal" },
    "../../../components/ui/ServiceLogo": { default: "ServiceLogo" },
    "../../../components/ui/Toast/ToastProvider": {
      useToast: () => ({ showToast: value => toasts.push(value) }),
    },
  });
  return {
    ...fixture,
    calls,
    toasts,
    credential,
    cancelled: () => cancelled,
    refreshed: () => refreshed,
  };
}

test("credential delete confirmation is standalone, compact, and read-only", async () => {
  const [details, dialog, page, styles] = await Promise.all([
    readFile(new URL("../src/features/vault/components/CredentialDetailsModal.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/features/vault/components/DeleteCredentialDialog.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/features/vault/VaultPage.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/styles/globals.css", import.meta.url), "utf8"),
  ]);

  assert.doesNotMatch(details, /isDeleting|deleteConfirmation|vault-delete-confirmation|Type .* to confirm/);
  assert.doesNotMatch(details, /vaultClient\.deleteCredential|Moved to Recently Deleted/);
  assert.match(details, /modalClose\.close\(\(\) => onRequestDelete\(credential\)\)/);
  assert.match(page, /credentialPendingDelete[\s\S]*<DeleteCredentialDialog/);
  assert.match(dialog, /width=\{500\}/);
  assert.match(dialog, /Delete credential\?[\s\S]*This credential will move to Recently Deleted for 30 days\./);
  assert.match(dialog, /<ServiceLogo[^>]*name=\{credential\.name\}[^>]*website=\{credential\.website\}[^>]*size="medium"/);
  assert.match(dialog, /credential-delete-preview-title[\s\S]*\{credential\.name\}[\s\S]*credential-delete-preview-type">Credential/);
  assert.doesNotMatch(dialog, /<(?:input|textarea)\b|Type .* to confirm|Delete Credential/);
  assert.match(styles, /\.notes-delete-preview,\s*\.credential-delete-preview\s*\{[^}]*border:\s*1px solid var\(--kn-border\);[^}]*border-radius:\s*10px;/);
  assert.doesNotMatch(styles, /vault-delete-confirmation/);
});

test("the details modal exits before handing the credential to confirmation", async () => {
  const fixture = await vaultFixture();
  await fixture.click("Delete credential");

  assert.equal(fixture.requestedDelete()?.id, "a");
  assert.equal(fixture.calls.some(([action]) => action === "delete"), false);
  assert.equal(fixture.closed(), false);
  fixture.unmount();
});

test("Cancel closes the confirmation without deleting", async () => {
  const fixture = await deleteDialog();
  fixture.find(node => node.type === "button" && node.props.children === "Cancel").props.onClick();
  await fixture.flush();

  assert.deepEqual(fixture.calls, []);
  assert.equal(fixture.cancelled(), 0);
  const modal = fixture.find(node => node.type === "VaultModal");
  assert.equal(modal.props.closing, true);
  modal.props.onExitComplete();
  await fixture.flush();
  assert.equal(fixture.cancelled(), 1);
  fixture.unmount();
});

test("Delete soft-deletes once, refreshes, and toasts only after success", async () => {
  const fixture = await deleteDialog();
  fixture.find(node => node.type === "button" && node.props.children === "Delete").props.onClick();
  await fixture.flush();

  assert.equal(JSON.stringify(fixture.calls), JSON.stringify([["delete", "a"]]));
  assert.equal(fixture.refreshed(), 1);
  assert.equal(
    JSON.stringify(fixture.toasts),
    JSON.stringify([{ type: "success", message: "Moved to Recently Deleted" }]),
  );
  const modal = fixture.find(node => node.type === "VaultModal");
  assert.equal(modal.props.closing, true);
  modal.props.onExitComplete();
  await fixture.flush();
  assert.equal(fixture.cancelled(), 1);
  fixture.unmount();
});
