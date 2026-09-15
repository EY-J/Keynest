import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("one shared toast owns lifecycle, restart, position, and reduced motion", async () => {
  const [provider, css, app, profile] = await Promise.all([
    readFile(new URL("../src/components/ui/Toast/ToastProvider.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/components/ui/Toast/toast.css", import.meta.url), "utf8"),
    readFile(new URL("../src/app/App.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/features/profile/ProfileSettings.tsx", import.meta.url), "utf8"),
  ]);

  assert.match(app, /<ToastProvider>[\s\S]*<KeyNestApp \/>/);
  assert.match(profile, /useToast\(\)/);
  assert.doesNotMatch(profile, /profile-save-toast|toastPhase|toastExitTimer/);
  assert.match(provider, /const TOAST_VISIBLE_MS = 2_200/);
  assert.match(provider, /clearToastTimers\(\);[\s\S]*setToastCycle|clearToastTimers\(\);[\s\S]*setCycle/);
  assert.match(provider, /setPhase\("exiting"\)[\s\S]*TOAST_EXIT_FALLBACK_MS/);
  assert.match(provider, /onAnimationEnd=\{finishExit\}/);
  assert.match(css, /\.keynest-toast\s*\{[^}]*position:\s*fixed;[^}]*right:\s*24px;[^}]*bottom:\s*24px;/s);
  assert.match(css, /\.keynest-toast\s*\{[^}]*width:\s*fit-content;[^}]*min-width:\s*min\(180px,\s*calc\(100vw - 48px\)\);[^}]*max-width:\s*min\(320px,\s*calc\(100vw - 48px\)\);/s);
  assert.match(css, /\.keynest-toast\s*\{[^}]*padding:\s*14px 18px;/s);
  assert.match(css, /\.keynest-toast\s*\{[^}]*text-align:\s*center;/s);
  assert.match(css, /\.keynest-toast (?:strong|span)\s*\{[^}]*overflow-wrap:\s*anywhere;[^}]*white-space:\s*normal;/s);
  assert.doesNotMatch(css, /width:\s*min\(260px/);
  assert.match(css, /\.keynest-toast\.is-visible\s*\{[^}]*200ms cubic-bezier\(0\.16,\s*1,\s*0\.3,\s*1\)/s);
  assert.match(css, /\.keynest-toast\.is-exiting\s*\{[^}]*200ms cubic-bezier\(0\.4,\s*0,\s*1,\s*1\)/s);
  assert.match(css, /translateY\(8px\) scale\(0\.98\)[\s\S]*translateY\(0\) scale\(1\)/);
  assert.match(css, /translateY\(0\) scale\(1\)[\s\S]*translateY\(6px\) scale\(0\.98\)/);
  assert.match(css, /@media \(prefers-reduced-motion: reduce\)[\s\S]*60ms linear/);
});

test("delete and restore flows emit exactly one success toast after storage succeeds", async () => {
  const [vault, notes, deleted] = await Promise.all([
    readFile(new URL("../src/features/vault/components/DeleteCredentialDialog.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/features/notes/NotesPage.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/features/recently-deleted/RecentlyDeletedPage.tsx", import.meta.url), "utf8"),
  ]);

  assert.match(vault, /await vaultClient\.deleteCredential[\s\S]*showToast\(\{ type: "success", message: "Moved to Recently Deleted" \}\)/);
  assert.match(notes, /await notesClient\.delete[\s\S]*showToast\(\{ type: "success", message: "Moved to Recently Deleted" \}\)/);
  assert.match(deleted, /await recentlyDeletedClient\.restore[\s\S]*message: "Item restored"/);
  assert.match(deleted, /await recentlyDeletedClient\.permanentlyDelete[\s\S]*message: "Item permanently deleted"/);
  assert.match(deleted, /await recentlyDeletedClient\.empty[\s\S]*message: "Recently Deleted emptied"/);
  assert.equal((vault.match(/Moved to Recently Deleted/g) ?? []).length, 1);
  assert.equal((notes.match(/Moved to Recently Deleted/g) ?? []).length, 1);
});

test("Recovery Key copy reports success and failure through the shared toast", async () => {
  const recovery = await readFile(
    new URL("../src/features/auth/components/RecoveryKeyScreen.tsx", import.meta.url),
    "utf8",
  );

  assert.match(recovery, /useToast\(\)/);
  assert.match(
    recovery,
    /await authClient\.copyRecoveryKey\(recoveryKey\);[\s\S]*showToast\(\{ type: "success", message: "Copied to clipboard" \}\)/,
  );
  assert.match(
    recovery,
    /catch \{[\s\S]*showToast\(\{ type: "error", message: "Unable to copy recovery key" \}\)/,
  );
  assert.doesNotMatch(recovery, /copyStatus|recovery-copy-status|Recovery Key copied temporarily/);
});

test("Offline Recovery screen keeps the shared confirmation concise and centered", async () => {
  const [recovery, css] = await Promise.all([
    readFile(new URL("../src/features/auth/components/RecoveryKeyScreen.tsx", import.meta.url), "utf8"),
    readFile(new URL("../src/features/auth/components/RecoveryKeyScreen.css", import.meta.url), "utf8"),
  ]);

  assert.match(recovery, /description="Your recovery key is generated once\. Store it somewhere safe\."/);
  assert.match(recovery, /<RecoveryKeyConfirmation \{\.\.\.props\} concise \/>/);
  assert.match(recovery, /keyGroups\.filter\(\(group\) => group\.length === 4\)/);
  assert.match(recovery, /globalThis\.crypto\.getRandomValues\(randomValue\)/);
  assert.match(recovery, /RECOVERY_GROUP_POSITION_NAMES\s*=\s*\[[\s\S]*"first"[\s\S]*"second"[\s\S]*"third"[\s\S]*"fourth"[\s\S]*"fifth"[\s\S]*"sixth"[\s\S]*"seventh"/);
  assert.match(recovery, /challengePosition === challengeCount[\s\S]*\? "final"[\s\S]*RECOVERY_GROUP_POSITION_NAMES\[challengeIndex\]/);
  assert.match(recovery, /Enter the \$\{challengePositionLabel\} 4-character group from your Recovery Key/);
  assert.match(recovery, /verification\.trim\(\)\.toUpperCase\(\) !== challengeGroup/);
  assert.match(recovery, /concise\s*\? "Confirm Recovery Key"/);
  assert.match(recovery, /\{!concise \? \([\s\S]*Keep this key somewhere separate[\s\S]*\) : null\}/);
  assert.match(css, /\.offline-recovery-step[\s\S]*text-align:\s*center;/);
  assert.match(css, /\.offline-recovery-step \.recovery-key-content\s*\{[^}]*gap:\s*12px;/s);
});
