import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("Recently Deleted is universal, searchable, and uses shared modal styling", async () => {
  const [page, client, dialog, sidebar] = await Promise.all([
    readFile(new URL("../src/features/recently-deleted/RecentlyDeletedPage.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/features/recently-deleted/recentlyDeletedClient.ts", import.meta.url), "utf8"),
    readFile(new URL("../src/features/recently-deleted/ConfirmPermanentDeleteDialog.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/app/components/NavigationSidebar.tsx", import.meta.url), "utf8"),
  ]);

  assert.match(sidebar, /Quick Access[\s\S]*Favorites[\s\S]*Recently Deleted/);
  assert.match(page, /Items are permanently deleted after 30 days\./);
  assert.match(page, /placeholder="Search deleted items\.\.\."/);
  assert.match(page, /className="recently-deleted-toolbar"[\s\S]*Search deleted items[\s\S]*Select all[\s\S]*Empty Bin/);
  assert.match(page, /className="vault-danger-button recently-deleted-empty-button"/);
  assert.doesNotMatch(page, /recently-deleted-heading[\s\S]*Empty Recently Deleted/);
  assert.match(page, /allVisibleSelected[\s\S]*Deselect all/);
  assert.match(page, /className="recently-deleted-row-select"/);
  assert.match(page, /Recently Deleted is empty/);
  assert.match(page, /Credential[\s\S]*Note/);
  assert.match(page, /Item restored/);
  assert.match(page, /Recently Deleted emptied/);
  assert.match(page, /Item permanently deleted/);
  assert.doesNotMatch(page, /recently-deleted-toast/);
  assert.doesNotMatch(page, /className="recently-deleted-restore"/);
  assert.match(page, /role="menuitem" onClick=\{\(\) => void restore\(item\)\}>Restore<\/button>/);
  assert.doesNotMatch(page, /item\.content|item\.username|item\.password/i);
  assert.match(client, /list_deleted_items/);
  assert.match(client, /restore_deleted_item/);
  assert.match(client, /permanently_delete_item/);
  assert.match(dialog, /<Modal[\s\S]*Delete permanently\?[\s\S]*cannot be restored/);
});
