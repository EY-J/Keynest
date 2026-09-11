import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { mount } from "./componentHarness.mjs";

const defaultProfile = {
  displayName: "KeyNest User",
  avatarDataUrl: null,
  hasCustomAvatar: false,
};
const customProfile = {
  displayName: "Míng 猫",
  avatarDataUrl: "data:image/png;base64,custom",
  hasCustomAvatar: true,
};
class ProfileClientError extends Error {}

function profileDependencies(profileClient, prepareProfileImage = async () => ({
  mimeType: "image/png",
  dataBase64: "staged",
  previewUrl: "data:image/png;base64,staged",
})) {
  return {
    "./ProfileSettings.css": {},
    "./ProfileAvatar": { default: "ProfileAvatar" },
    "./profileClient": { profileClient, prepareProfileImage },
    "./profileTypes": { ProfileClientError },
  };
}

test("sidebar profile is display-only and contains no edit affordance", async () => {
  const f = await mount("../src/app/components/NavigationSidebar.tsx", {
    isOpen: true, activeDestination: "home", onClose() {}, onNavigate() {},
    async onLockKeynest() {}, profile: customProfile,
  }, {
    "lucide-react": { FolderLock: "i", House: "i", KeyRound: "i", Lock: "i", NotebookPen: "i", Settings: "i", Star: "i" },
    "../../features/profile/ProfileAvatar": { default: "ProfileAvatar" },
    "../../features/profile/profileTypes": {},
  });
  const profile = f.find(node => node.props.className === "sidebar-profile");
  assert.equal(profile.type, "div");
  assert.equal(profile.props.onClick, undefined);
  assert.doesNotMatch(JSON.stringify(profile), /Pencil|Edit profile|dialog/);
  f.unmount();
});

test("Settings uses Profile first and has no redundant SETTINGS kicker", async () => {
  const f = await mount("../src/features/settings/SettingsPage.tsx", {
    profile: customProfile, onProfileSaved() {}, async onResetAuthenticated() {},
  }, {
    "lucide-react": { Info: "i", Palette: "i", Shield: "i", Settings: "i", UserRound: "i" },
    "../profile/ProfileSettings": { default: "ProfileSettings" },
    "../profile/profileTypes": {},
    "./components/AboutSettings": { default: "AboutSettings" },
    "./components/AppearanceSettings": { default: "AppearanceSettings" },
    "./components/GeneralSettings": { default: "GeneralSettings" },
    "./components/SecuritySettings": { default: "SecuritySettings" },
  });
  const tabs = f.nodes().filter(node => node.props.role === "tab");
  assert.equal(tabs[0].props.children[1].props.children, "Profile");
  assert.equal(f.find(node => node.props.className === "settings-kicker"), undefined);
  assert.equal(f.find(node => node.type === "p").props.children, "Manage your local profile.");
  assert.equal(f.find(node => node.type === "ProfileSettings").props.profile, customProfile);
  f.unmount();
});

test("Profile uses one compact panel instead of separate SettingsRow cards", async () => {
  const f = await mount("../src/features/profile/ProfileSettings.tsx", {
    profile: customProfile, onSaved() {},
  }, profileDependencies({ async saveProfile() { throw new Error("not called"); } }));
  const panels = f.nodes().filter(node => node.props.className === "profile-settings-panel");
  assert.equal(panels.length, 1);
  assert.equal(f.nodes().some(node => node.type === "SettingsRow"), false);
  assert.equal(f.find(node => node.type === "h2" && node.props.children === "Profile photo").type, "h2");
  assert.equal(f.find(node => node.type === "input" && node.props.type === "text").props.value, customProfile.displayName);
  assert.equal(f.find(node => node.props.children === "Save changes").props.disabled, true);
  const photoActions = f.find(node => node.props.className === "profile-settings-photo-actions");
  assert.equal(
    photoActions.props.children.flat().filter(Boolean).map(node => node.props.children).join("|"),
    "Remove photo|Change photo",
  );
  const nameRow = f.find(node => node.props.className === "profile-settings-name-row");
  assert.equal(nameRow.props.children.map(node => node.type).join("|"), "input|button");
  f.unmount();
});

test("Profile panel keeps the requested compact dimensions and responsive stack", async () => {
  const css = await readFile(new URL("../src/features/profile/ProfileSettings.css", import.meta.url), "utf8");
  assert.match(css, /\.profile-settings\s*\{[^}]*width:\s*min\(100%,\s*700px\);/s);
  assert.match(css, /\.profile-settings-panel\s*\{[^}]*padding:\s*23px;/s);
  assert.match(css, /\.profile-settings-photo\s*\{[^}]*display:\s*flex;[^}]*align-items:\s*center;[^}]*justify-content:\s*space-between;/s);
  assert.match(css, /\.profile-settings-photo-identity\s*\{[^}]*display:\s*flex;[^}]*align-items:\s*center;[^}]*gap:\s*14px;/s);
  assert.match(css, /\.profile-settings-avatar\s*\{[^}]*width:\s*60px;[^}]*height:\s*60px;/s);
  assert.match(css, /\.profile-settings-name-row\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\) auto;[^}]*gap:\s*12px;/s);
  assert.match(css, /\.settings-page :is\(\.profile-settings-change-photo, \.profile-settings-save\)\s*\{[^}]*width:\s*auto;[^}]*min-width:\s*130px;[^}]*height:\s*40px;/s);
  assert.match(css, /@media\s*\(max-width:\s*620px\)[\s\S]*?\.profile-settings-photo\s*\{[^}]*flex-direction:\s*column;[\s\S]*?\.profile-settings-name-row\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\);/);
  assert.doesNotMatch(css, /profile-settings-(?:footer|photo-content)/);
});

test("successful saves use one restartable floating toast with a clean exit", async () => {
  let saves = 0;
  const f = await mount("../src/features/profile/ProfileSettings.tsx", {
    profile: defaultProfile, onSaved() {},
  }, profileDependencies({
    async saveProfile(displayName) {
      saves++;
      return { ...defaultProfile, displayName };
    },
  }));

  const nameInput = () => f.find(node => node.type === "input" && node.props.type === "text");
  nameInput().props.onChange({ currentTarget: { value: "First save" } });
  await f.flush();
  f.find(node => node.type === "form").props.onSubmit({ preventDefault() {} });
  await f.flush();

  const firstToast = f.find(node => node.props.className === "profile-save-toast is-visible is-success");
  assert.equal(firstToast.props.role, "status");
  assert.equal(firstToast.props["aria-live"], "polite");
  assert.equal(firstToast.props.children[0].props.children, "Profile saved");
  assert.doesNotMatch(JSON.stringify(f.find(node => node.props.className === "profile-settings-panel")), /Profile saved/);
  assert.equal(f.find(node => node.props.children === "Profile saved."), undefined);
  assert.deepEqual([...f.timers.values()], [2_200]);

  nameInput().props.onChange({ currentTarget: { value: "Second save" } });
  await f.flush();
  f.find(node => node.type === "form").props.onSubmit({ preventDefault() {} });
  await f.flush();
  const secondToast = f.find(node => node.props.className === "profile-save-toast is-visible is-success");
  assert.equal(saves, 2);
  assert.notEqual(secondToast.key, firstToast.key);
  assert.deepEqual([...f.timers.values()], [2_200]);

  f.expire();
  await f.flush();
  const exitingToast = f.find(node => node.props.className === "profile-save-toast is-exiting is-success");
  assert.equal(exitingToast.props.role, "status");
  assert.deepEqual([...f.timers.values()], [250]);
  exitingToast.props.onAnimationEnd({ currentTarget: exitingToast, target: exitingToast });
  await f.flush();
  assert.equal(f.find(node => String(node.props.className).startsWith("profile-save-toast")), undefined);
  assert.equal(f.timers.size, 0);

  nameInput().props.onChange({ currentTarget: { value: "Third save" } });
  await f.flush();
  f.find(node => node.type === "form").props.onSubmit({ preventDefault() {} });
  await f.flush();
  assert.equal(f.timers.size, 1);
  f.unmount();
  assert.equal(f.timers.size, 0);
});

test("empty display-name validation uses the shared error toast and never saves", async () => {
  let saves = 0;
  const f = await mount("../src/features/profile/ProfileSettings.tsx", {
    profile: defaultProfile, onSaved() {},
  }, profileDependencies({
    async saveProfile() {
      saves++;
      return defaultProfile;
    },
  }));

  const nameInput = f.find(node => node.type === "input" && node.props.type === "text");
  nameInput.props.onChange({ currentTarget: { value: "   " } });
  await f.flush();
  f.find(node => node.type === "form").props.onSubmit({ preventDefault() {} });
  await f.flush();

  const toast = f.find(node => node.props.className === "profile-save-toast is-visible is-error");
  assert.equal(saves, 0);
  assert.equal(toast.props.role, "status");
  assert.equal(toast.props.children[0].props.children, "Enter a display name.");
  assert.equal(toast.props.children[1], null);
  assert.equal(f.find(node => node.props.role === "alert"), undefined);
  assert.doesNotMatch(
    JSON.stringify(f.find(node => node.props.className === "profile-settings-panel")),
    /Enter a display name\./,
  );
  assert.deepEqual([...f.timers.values()], [2_200]);
  f.unmount();
  assert.equal(f.timers.size, 0);
});

test("Profile save toast is compact, responsive, and animated outside layout flow", async () => {
  const css = await readFile(new URL("../src/features/profile/ProfileSettings.css", import.meta.url), "utf8");
  assert.match(css, /\.profile-save-toast\s*\{[^}]*position:\s*fixed;[^}]*right:\s*24px;[^}]*bottom:\s*24px;/s);
  assert.match(css, /\.profile-save-toast\s*\{[^}]*width:\s*min\(260px,\s*calc\(100vw - 48px\)\);/s);
  assert.match(css, /\.profile-save-toast\.is-success\s*\{[^}]*var\(--kn-accent\)/s);
  assert.match(css, /\.profile-save-toast\.is-error\s*\{[^}]*var\(--kn-danger\)/s);
  assert.match(css, /\.profile-save-toast\.is-visible\s*\{[^}]*profile-save-toast-enter 200ms cubic-bezier\(0\.16,\s*1,\s*0\.3,\s*1\) both;/s);
  assert.match(css, /\.profile-save-toast\.is-exiting\s*\{[^}]*profile-save-toast-exit 200ms cubic-bezier\(0\.4,\s*0,\s*1,\s*1\) both;/s);
  assert.match(css, /@keyframes profile-save-toast-enter\s*\{[\s\S]*?translateY\(8px\) scale\(0\.98\)[\s\S]*?translateY\(0\) scale\(1\)/);
  assert.match(css, /@keyframes profile-save-toast-exit\s*\{[\s\S]*?translateY\(0\) scale\(1\)[\s\S]*?translateY\(6px\) scale\(0\.98\)/);
  assert.match(css, /@media \(prefers-reduced-motion: reduce\)[\s\S]*?profile-save-toast-fade-in 60ms linear both;[\s\S]*?profile-save-toast-fade-out 60ms linear both;/);
});

test("file-picker cancellation returns before any profile state changes", async () => {
  let preparations = 0, saves = 0;
  const f = await mount("../src/features/profile/ProfileSettings.tsx", {
    profile: customProfile, onSaved() {},
  }, profileDependencies(
    { async saveProfile() { saves++; return customProfile; } },
    async () => { preparations++; throw new Error("not called"); },
  ));
  const input = f.find(node => node.type === "input" && node.props.type === "file");
  const picker = { files: [], value: "unchanged" };
  await input.props.onChange({ currentTarget: picker });
  await f.flush();
  assert.equal(picker.value, "unchanged");
  assert.equal(f.find(node => node.type === "ProfileAvatar").props.avatarUrl, customProfile.avatarDataUrl);
  assert.equal(preparations, 0);
  assert.equal(saves, 0);
  f.unmount();
});

test("valid photo previews before Save and then persists", async () => {
  const calls = [];
  const f = await mount("../src/features/profile/ProfileSettings.tsx", {
    profile: defaultProfile, onSaved() {},
  }, profileDependencies({
    async saveProfile(name, avatarUpdate) {
      calls.push({ name, avatarUpdate });
      return { ...defaultProfile, avatarDataUrl: "data:image/png;base64,saved", hasCustomAvatar: true };
    },
  }));
  const input = f.find(node => node.type === "input" && node.props.type === "file");
  await input.props.onChange({ currentTarget: { files: [{ name: "avatar.png" }], value: "chosen" } });
  await f.flush();
  assert.equal(calls.length, 0);
  assert.equal(f.find(node => node.type === "ProfileAvatar").props.avatarUrl, "data:image/png;base64,staged");
  f.find(node => node.type === "form").props.onSubmit({ preventDefault() {} });
  await f.flush();
  assert.equal(calls[0].avatarUpdate.kind, "replace");
  f.unmount();
});

test("Remove photo stages the bundled default until Save", async () => {
  const calls = [];
  const f = await mount("../src/features/profile/ProfileSettings.tsx", {
    profile: customProfile, onSaved() {},
  }, profileDependencies({
    async saveProfile(name, avatarUpdate) {
      calls.push({ name, avatarUpdate });
      return defaultProfile;
    },
  }));
  f.find(node => node.props.children === "Remove photo").props.onClick();
  await f.flush();
  assert.equal(calls.length, 0);
  assert.equal(f.find(node => node.type === "ProfileAvatar").props.avatarUrl, null);
  f.find(node => node.type === "form").props.onSubmit({ preventDefault() {} });
  await f.flush();
  assert.equal(calls[0].avatarUpdate.kind, "remove");
  f.unmount();
});

test("profile persistence stays isolated from authentication and vault code", async () => {
  const rust = await readFile(new URL("../src-tauri/src/profile.rs", import.meta.url), "utf8");
  const client = await readFile(new URL("../src/features/profile/profileClient.ts", import.meta.url), "utf8");
  assert.doesNotMatch(rust, /AuthService|VaultService|AutofillService|Master Password|Recovery Key/);
  assert.doesNotMatch(client, /authClient|vaultClient|autofill|fetch\s*\(/);
  assert.match(rust, /app_data_dir\.join\(PROFILE_DIRECTORY\)/);
});
