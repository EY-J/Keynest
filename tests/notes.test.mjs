import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import { transformWithOxc } from "vite";

async function noteUtils() {
  const file = new URL("../src/features/notes/noteUtils.ts", import.meta.url);
  const { code } = await transformWithOxc(await readFile(file, "utf8"), file.pathname);
  const module = new vm.SourceTextModule(code);
  await module.link(() => assert.fail("Note utilities must stay dependency-free at runtime"));
  await module.evaluate();
  return module.namespace;
}

const notes = [
  { id: "a", title: "Project ideas", content: "Improve KeyNest", tags: ["Ideas", "Work"], favorite: false, createdAtMs: 10, updatedAtMs: 20 },
  { id: "b", title: "Groceries", content: "Tea and oranges", tags: ["Personal"], favorite: true, createdAtMs: 30, updatedAtMs: 40 },
  { id: "c", title: "Japan", content: "Visit Kyoto", tags: ["Travel", "Ideas"], favorite: false, createdAtMs: 20, updatedAtMs: 30 },
];

test("Notes search covers title and content while preserving the list order", async () => {
  const utils = await noteUtils();
  assert.deepEqual(utils.filterNotes(notes, "project").map(note => note.id), ["a"]);
  assert.deepEqual(utils.filterNotes(notes, "oranges").map(note => note.id), ["b"]);
  assert.deepEqual(utils.filterNotes(notes, "").map(note => note.id), ["a", "b", "c"]);
});

test("Notes uses the minimal sidebar, rows, editor, and shared delete modal", async () => {
  const [toolbar, list, page, editor, dialog, styles] = await Promise.all([
    readFile(new URL("../src/features/notes/components/NotesToolbar.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/features/notes/components/NotesList.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/features/notes/NotesPage.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/features/notes/components/NoteEditorPanel.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/features/notes/components/DeleteNoteDialog.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/styles/globals.css", import.meta.url), "utf8"),
  ]);
  assert.match(toolbar, /className="vault-search-control"/);
  assert.match(toolbar, /placeholder="Search notes\.\.\."/);
  assert.doesNotMatch(toolbar, /<Select|All tags|Recently updated|Filter/);
  assert.match(toolbar, /aria-label="New note"/);
  assert.doesNotMatch(list, /tag|badge|favorite|Star/);
  assert.match(list, /notes-list-preview/);
  assert.match(list, /<time/);
  assert.match(page, /className="password-vault-page notes-page"/);
  assert.match(page, /showToast\(\{ type: "success", message: "Moved to Recently Deleted" \}\)/);
  assert.doesNotMatch(page, /notes-list-heading|visibleNotes\.length|sort|tag=/);
  assert.match(editor, /No note selected/);
  assert.match(editor, /Select a note or create a new one\./);
  assert.match(editor, /}, 650\);/);
  assert.match(editor, /Saving\.\.\.|Saved/);
  assert.match(dialog, /<Modal[\s\S]*?Delete note\?[\s\S]*?Recently Deleted for 30 days/);
  assert.match(dialog, /<FileText[\s\S]*?notes-delete-preview-title[\s\S]*?\{note\.title\}[\s\S]*?notes-delete-preview-type">Note</);
  assert.doesNotMatch(dialog, /<(?:input|textarea)\b/);
  assert.doesNotMatch(dialog, /notes-delete-name/);
  assert.match(styles, /\.notes-delete-preview,\s*\.credential-delete-preview\s*\{[^}]*display:\s*flex;[^}]*align-items:\s*center;[^}]*gap:\s*12px;/);
  assert.match(styles, /\.notes-delete-preview-title,\s*\.credential-delete-preview-title\s*\{[^}]*overflow:\s*hidden;[^}]*text-overflow:\s*ellipsis;[^}]*white-space:\s*nowrap;/);
});
