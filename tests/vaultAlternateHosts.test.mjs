import assert from "node:assert/strict";
import test from "node:test";
import { mount } from "./componentHarness.mjs";
import { summary } from "./vaultFixture.mjs";

const dependencies = {
  "../passwordGenerator": { generateAdvancedPassword: () => "generated-fixture" },
  "../../../components/ui/PasswordStrengthMeter": {
    default: "PasswordStrengthMeter",
  },
};

function assertNormalWebsiteOnly(form) {
  assert.equal(form.find(node => node.props.id === "vault-allowed-login-hosts"), undefined);
  assert.equal(form.find(node => node.props.id === "vault-allowed-login-hosts-hint"), undefined);
  assert.ok(!form.nodes().some(node =>
    node.type === "span" && node.props.children === "Allowed login hosts (advanced)"));
  const urlInputs = form.nodes().filter(node => node.type === "input" && node.props.inputMode === "url");
  assert.equal(urlInputs.length, 1);
  assert.equal(urlInputs[0].props.placeholder, "e.g. https://google.com");
  return urlInputs[0];
}

function change(form, predicate, value) {
  const field = form.find(predicate);
  assert.ok(field);
  field.props.onChange({ target: { value } });
}

test("normal Add Credential UI shows Website only and new records submit no alternate hosts", async () => {
  let submitted;
  const form = await mount(
    "../src/features/vault/components/CredentialForm.tsx",
    {
      onSubmit: async input => { submitted = input; },
      onCancel() {},
    },
    dependencies,
  );

  assertNormalWebsiteOnly(form).props.onChange({ target: { value: "https://facebook.com" } });
  change(form, node => node.props.id === "vault-name", "Social account");
  change(form, node => node.props.id === "vault-username", "fixture-user");
  change(form, node => node.props.id === "vault-password", "fixture-password");
  await form.flush();
  form.find(node => node.type === "form").props.onSubmit({ preventDefault() {} });
  await form.flush();

  assert.equal(submitted.website, "https://facebook.com");
  assert.deepEqual(Array.from(submitted.allowedLoginHosts), []);
  form.unmount();
});

test("normal Edit Credential UI preserves hidden alternate hosts across all ordinary edits", async () => {
  let submitted;
  const form = await mount(
    "../src/features/vault/components/CredentialForm.tsx",
    {
      initialRecord: {
        ...summary("alternate-host"),
        password: "editable-secret",
        website: "https://gmail.com/",
        allowedLoginHosts: ["accounts.google.com", "login.microsoftonline.com"],
      },
      onSubmit: async input => { submitted = input; },
      onCancel() {},
    },
    dependencies,
  );

  const website = assertNormalWebsiteOnly(form);
  change(form, node => node.props.id === "vault-name", "Updated account");
  change(form, node => node.props.id === "vault-username", "updated-user");
  change(form, node => node.props.id === "vault-password", "updated-password");
  website.props.onChange({ target: { value: "https://mail.google.com/" } });
  change(form, node => node.props.placeholder === "e.g. work, personal", "personal, updated");
  await form.flush();
  form.find(node => node.type === "form").props.onSubmit({ preventDefault() {} });
  await form.flush();

  assert.equal(submitted.name, "Updated account");
  assert.equal(submitted.username, "updated-user");
  assert.equal(submitted.password, "updated-password");
  assert.equal(submitted.website, "https://mail.google.com/");
  assert.deepEqual(Array.from(submitted.tags), ["personal", "updated"]);
  assert.deepEqual(Array.from(submitted.allowedLoginHosts), [
    "accounts.google.com",
    "login.microsoftonline.com",
  ]);
  form.unmount();
});
