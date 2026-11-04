import assert from "node:assert/strict";
import test from "node:test";
import { mount } from "./componentHarness.mjs";

const credential = { id: "credential-1", name: "Account" };
const note = { id: "note-1", title: "Ideas" };

test("Folders page creates, renames, displays both item types, and confirms organizational deletion", async () => {
  let folders = [], items = [], serial = 0;
  const calls = [], destinations = [];
  const client = {
    list: async () => [...folders],
    items: async () => [...items],
    create: async name => {
      calls.push(["create", name]);
      const folder = { id: `folder-${++serial}`, name, createdAtMs: 1, updatedAtMs: 1 };
      folders = [...folders, folder]; return folder;
    },
    rename: async (id, name) => {
      calls.push(["rename", id, name]);
      folders = folders.map(folder => folder.id === id ? { ...folder, name } : folder);
      return folders.find(folder => folder.id === id);
    },
    delete: async id => {
      calls.push(["delete", id]);
      folders = folders.filter(folder => folder.id !== id);
      items = items.filter(item => item.folderId !== id);
    },
  };
  const f = await mount("../src/features/folders/FoldersPage.tsx", {}, {
    "lucide-react": { FileText: "Icon", Folder: "Icon", KeyRound: "Icon", Plus: "Icon", X: "Icon" },
    "react-router-dom": { useNavigate: () => (path, options) => destinations.push([path, options]) },
    "../vault/vaultClient": { vaultClient: { listCredentials: async () => [credential] } },
    "../notes/notesClient": { notesClient: { listSummaries: async () => [note] } },
    "./foldersClient": { foldersClient: client },
  });
  assert.ok(f.find(node => node.props.className === "vault-empty-state" && f.nodes(node).some(child => child.props.children === "No folders yet")));
  assert.equal(f.nodes(f.find(node => node.props.className === "vault-empty-state")).some(node => node.type === "button"), false);
  f.find(node => node.type === "button" && node.props.className === "primary-button").props.onClick();
  await f.flush();
  f.find(node => node.type === "input" && node.props.id === "folder-name").props.onChange({ target: { value: "  Work  " } });
  await f.flush();
  f.find(node => node.type === "form").props.onSubmit({ preventDefault() {} });
  await f.flush();
  assert.deepEqual(calls[0], ["create", "Work"]);
  f.find(node => node.props.titleId === "folder-form-title").props.onExitComplete();
  await f.flush();
  assert.ok(f.find(node => node.props.className?.includes?.("folders-list-item")));
  items = [
    { kind: "credential", itemId: credential.id, folderId: folders[0].id },
    { kind: "note", itemId: note.id, folderId: folders[0].id },
  ];
  // The page reloads memberships on a new mount, as it does when navigating back from Vault/Notes.
  f.unmount();
  const g = await mount("../src/features/folders/FoldersPage.tsx", {}, {
    "lucide-react": { FileText: "Icon", Folder: "Icon", KeyRound: "Icon", Plus: "Icon", X: "Icon" },
    "react-router-dom": { useNavigate: () => (path, options) => destinations.push([path, options]) },
    "../vault/vaultClient": { vaultClient: { listCredentials: async () => [credential] } },
    "../notes/notesClient": { notesClient: { listSummaries: async () => [note] } },
    "./foldersClient": { foldersClient: client },
  });
  g.find(node => node.props.className?.includes?.("folders-list-item")).props.onClick(); await g.flush();
  assert.equal(g.nodes().filter(node => node.props.className === "folders-content-item").length, 2);
  assert.ok(g.nodes().some(node => node.props.children === "Credentials"));
  assert.ok(g.nodes().some(node => node.props.children === "Notes"));
  g.find(node => node.props.className === "folders-content-item" && node.props.children[1].props.children === "Account").props.onClick();
  assert.equal(destinations[0][0], "/vault");
  g.find(node => node.type === "button" && node.props.children === "Rename").props.onClick(); await g.flush();
  g.find(node => node.type === "input" && node.props.id === "folder-name").props.onChange({ target: { value: "Office" } }); await g.flush();
  g.find(node => node.type === "form").props.onSubmit({ preventDefault() {} }); await g.flush();
  assert.deepEqual(calls.at(-1), ["rename", folders[0].id, "Office"]);
  g.find(node => node.props.titleId === "folder-form-title").props.onExitComplete(); await g.flush();
  g.find(node => node.type === "button" && node.props.children === "Delete").props.onClick(); await g.flush();
  assert.ok(g.nodes().some(node => typeof node.props.children === "string" && node.props.children.includes("Items in this folder will become unfiled")));
  g.find(node => node.type === "button" && node.props.children === "Delete folder").props.onClick(); await g.flush();
  assert.deepEqual(calls.at(-1), ["delete", "folder-1"]);
  assert.deepEqual(items, []);
  g.unmount();
});

test("folder assignment moves credentials and notes between folders and unfiles them", async () => {
  const folders = [{ id: "a", name: "Work" }, { id: "b", name: "Personal" }];
  const memberships = [{ kind: "credential", itemId: "c", folderId: "a" }];
  const calls = [];
  const client = {
    list: async () => folders,
    items: async () => memberships,
    assignCredential: async (id, folderId) => calls.push(["credential", id, folderId]),
    assignNote: async (id, folderId) => calls.push(["note", id, folderId]),
  };
  const deps = { "./foldersClient": { foldersClient: client } };
  const credentialView = await mount("../src/features/folders/FolderAssignment.tsx", { kind: "credential", itemId: "c" }, deps);
  const select = () => credentialView.find(node => node.type === "select");
  assert.equal(select().props.value, "a");
  select().props.onChange({ target: { value: "b" } }); await credentialView.flush();
  select().props.onChange({ target: { value: "" } }); await credentialView.flush();
  assert.deepEqual(calls, [["credential", "c", "b"], ["credential", "c", null]]);
  credentialView.unmount();
  let assigned = 0;
  const noteView = await mount("../src/features/folders/FolderAssignment.tsx", { kind: "note", itemId: "n", onAssigned: () => assigned++ }, deps);
  noteView.find(node => node.type === "select").props.onChange({ target: { value: "a" } }); await noteView.flush();
  assert.deepEqual(calls.at(-1), ["note", "n", "a"]);
  assert.equal(assigned, 1);
  noteView.unmount();
});

test("note folder picker opens only from Add to folder in the actions menu", async () => {
  const note = {
    id: "note-1", title: "Ideas", content: "Private text", tags: [], favorite: false,
    createdAtMs: 1, updatedAtMs: 1,
  };
  const f = await mount("../src/features/notes/components/NoteEditorPanel.tsx", {
    note, onSaved() {}, onDelete() {}, onDuplicate() {}, onToggleFavorite() {},
  }, {
    "lucide-react": Object.fromEntries(["Copy", "FolderPlus", "MoreHorizontal", "Star", "Trash2"].map(name => [name, "Icon"])),
    "../../../components/ui/eye": { EyeIcon: "EyeIcon" },
    "../../../components/ui/square-pen": { SquarePenIcon: "SquarePenIcon" },
    "../notesClient": { notesClient: { update: async () => note } },
    "../../folders/FolderAssignmentDialog": { default: "FolderAssignmentDialog" },
  });
  assert.equal(f.find(node => node.type === "FolderAssignmentDialog"), undefined);
  assert.equal(f.find(node => node.props.className === "folder-assignment"), undefined);
  f.find(node => node.props["aria-label"] === "Note actions").props.onClick(); await f.flush();
  const menu = f.find(node => node.props["aria-label"] === "Note actions" && node.props.role === "menu");
  const action = f.nodes(menu).find(node => node.type === "button" && f.nodes(node).some(child => child.props.children === "Add to folder"));
  action.props.onClick(); await f.flush();
  const dialog = f.find(node => node.type === "FolderAssignmentDialog");
  assert.equal(dialog.props.itemId, note.id);
  assert.equal(dialog.props.kind, "note");
  dialog.props.onClose(); await f.flush();
  assert.equal(f.find(node => node.type === "FolderAssignmentDialog"), undefined);
  f.unmount();
});
