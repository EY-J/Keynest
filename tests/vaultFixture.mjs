import { mount } from "./componentHarness.mjs";

export const summary = id => ({
  id,
  name: `Account ${id}`,
  username: "fixture-user",
  tags: ["work"],
  website: "https://example.test",
  allowedLoginHosts: [],
  createdAtMs: 1_700_000_000_000,
  updatedAtMs: 1_710_000_000_000,
});
export async function vaultFixture(
  getSecret = async id => ({ ...summary(id), password: `fixture-secret-${id}` }),
  getSummary = async id => summary(id),
) {
  const calls = [];
  let closed = false;
  const f = await mount("../src/features/vault/components/VaultRecordDialog.tsx", {
    recordId: "a", isFavorite: false,
    onClose: () => { closed = true; }, onChanged: async () => {},
    onToggleFavorite: () => { calls.push(["favorite", "a"]); },
  }, {
    "../vaultClient": { vaultClient: {
      getVaultRecordSummary: async id => { calls.push(["summary", id]); return getSummary(id); },
      getVaultRecord: id => { calls.push(["secret", id]); return getSecret(id); },
      copyVaultPassword: async id => { calls.push(["copy", id]); },
      copyVaultUsername: async id => { calls.push(["copy-username", id]); },
      updateVaultRecord: async (id, input) => { calls.push(["update", id, input]); },
    } },
    "@tauri-apps/plugin-opener": {
      openUrl: async value => { calls.push(["open", value.toString()]); },
    },
    "./VaultModal": { default: "VaultModal" },
    "./VaultRecordForm": { default: "VaultRecordForm" },
    "../../../shared/components/ServiceIcon": { default: "ServiceIcon" },
    "lucide-react": {
      Check: "Check", Copy: "Copy", Eye: "Eye", EyeOff: "EyeOff",
      ExternalLink: "ExternalLink", Link2: "Link2", LockKeyhole: "LockKeyhole",
      Pencil: "Pencil", Star: "Star", Trash2: "Trash2", UserRound: "UserRound", X: "X",
    },
  }, { globals: { URL } });
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
