import { mount } from "./componentHarness.mjs";

export const summary = id => ({ id, name: `Account ${id}`, username: "fixture-user", tags: [], website: null });
export async function vaultFixture(getSecret = async id => ({ ...summary(id), password: `fixture-secret-${id}` })) {
  const calls = [];
  let closed = false;
  const f = await mount("../src/features/vault/components/VaultRecordDialog.tsx", {
    recordId: "a", onClose: () => { closed = true; }, onChanged: async () => {},
  }, {
    "../vaultClient": { vaultClient: {
      getVaultRecordSummary: async id => { calls.push(["summary", id]); return summary(id); },
      getVaultRecord: id => { calls.push(["secret", id]); return getSecret(id); },
      copyVaultPassword: async id => { calls.push(["copy", id]); },
      updateVaultRecord: async (id, input) => { calls.push(["update", id, input]); },
    } },
    "./VaultModal": { default: "VaultModal" },
    "./VaultRecordForm": { default: "VaultRecordForm" },
  });
  return { ...f, calls, closed: () => closed,
    async click(label) {
      f.find(n => n.type === "button" && (n.props.children === label || n.props["aria-label"] === label)).props.onClick(); await f.flush();
      // Adapter port: shared Modal timing is covered in modal.test.mjs.
      const modal = f.find(n => n.type === "VaultModal");
      if (modal?.props.closing) { modal.props.onExitComplete(); await f.flush(); }
    },
    password: () => f.find(n => n.props["aria-label"] === "Password")?.props,
  };
}
