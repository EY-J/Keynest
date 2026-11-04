import assert from "node:assert/strict";
import test from "node:test";
import { mount } from "./componentHarness.mjs";

const defaultProfile = {
  displayName: "",
  avatarDataUrl: null,
  hasCustomAvatar: false,
  isConfigured: false,
};

const savedProfile = {
  displayName: "KeyNest User",
  avatarDataUrl: null,
  hasCustomAvatar: false,
  isConfigured: true,
};

const icons = Object.fromEntries([
  "FileText", "Folder", "House", "KeyRound", "Lock", "LockKeyhole",
  "NotebookPen", "Settings", "Sparkles", "Star", "Trash2",
].map((name) => [name, "i"]));

function sidebarDependencies() {
  return {
    "lucide-react": icons,
    "react-router-dom": { NavLink: "NavLink" },
    "../../features/profile/ProfileAvatar": { default: "ProfileAvatar" },
    "../../features/profile/profileTypes": {},
  };
}

function appShellDependencies({
  profile = Promise.resolve(savedProfile),
  settings = {},
  initialFavorites = new Set(["saved-favorite"]),
} = {}) {
  return {
    "react-router-dom": { Navigate: "Navigate", Route: "Route", Routes: "Routes" },
    "../features/settings/SettingsProvider": { useSettings: () => ({ settings }) },
    "../features/autofill/components/HostApprovalModal": { default: "HostApprovalModal" },
    "../features/vault/credentialFavorites": {
      loadFavoriteCredentialIds: () => new Set(initialFavorites),
      saveFavoriteCredentialIds() {},
      toggleFavoriteCredentialId(current, id) {
        const next = new Set(current);
        if (next.has(id)) next.delete(id); else next.add(id);
        return next;
      },
    },
    "../features/home/HomePage": { default: "HomePage" },
    "../features/folders/FoldersPage": { default: "FoldersPage" },
    "../features/notes/NotesPage": { default: "NotesPage" },
    "../features/recently-deleted/RecentlyDeletedPage": { default: "RecentlyDeletedPage" },
    "../features/vault/VaultPage": { default: "VaultPage" },
    "../features/settings/SettingsPage": { default: "SettingsPage" },
    "../features/profile/profileClient": {
      profileClient: { getProfile: () => typeof profile === "function" ? profile() : profile },
    },
    "../features/profile/profileTypes": { DEFAULT_PROFILE: defaultProfile },
    "./components/AppTitleBar": { default: "AppTitleBar" },
    "./components/NavigationSidebar": { default: "NavigationSidebar" },
  };
}

test("AppShell maps every hash route and redirects root or unknown routes to Home", async () => {
  const resetAuthenticated = async () => {};
  const f = await mount("../src/app/AppShell.tsx", {
    async onLockKeynest() {}, onResetAuthenticated: resetAuthenticated,
  }, appShellDependencies());
  const routes = f.nodes().filter((node) => node.type === "Route");
  assert.deepEqual(routes.map((route) => route.props.path), [
    "/", "/home", "/vault", "/notes", "/folders", "/favorites",
    "/recently-deleted", "/settings", "*",
  ]);
  const byPath = Object.fromEntries(routes.map((route) => [route.props.path, route.props.element]));
  assert.deepEqual([byPath["/"].type, byPath["/"].props.to, byPath["/"].props.replace], ["Navigate", "/home", true]);
  assert.equal(byPath["/home"].type, "HomePage");
  assert.equal(byPath["/notes"].type, "NotesPage");
  assert.equal(byPath["/folders"].type, "FoldersPage");
  assert.equal(byPath["/recently-deleted"].type, "RecentlyDeletedPage");
  assert.equal(byPath["/settings"].type, "SettingsPage");
  assert.equal(byPath["/settings"].props.onResetAuthenticated, resetAuthenticated);
  assert.deepEqual([byPath["/vault"].type, byPath["/vault"].props.favoritesOnly], ["VaultPage", false]);
  assert.deepEqual([byPath["/favorites"].type, byPath["/favorites"].props.favoritesOnly], ["VaultPage", true]);
  assert.deepEqual([...byPath["/vault"].props.favoriteCredentialIds], ["saved-favorite"]);
  assert.equal(byPath["/favorites"].props.favoriteCredentialIds, byPath["/vault"].props.favoriteCredentialIds);
  assert.deepEqual([byPath["*"].type, byPath["*"].props.to, byPath["*"].props.replace], ["Navigate", "/home", true]);
  f.unmount();
});

test("Home actions navigate to every available route", async () => {
  const destinations = [];
  const f = await mount("../src/features/home/HomePage.tsx", {}, {
    "lucide-react": icons,
    "react-router-dom": { useNavigate: () => (path) => destinations.push(path) },
    "../../components/ui/BrandMark": { default: "BrandMark" },
  });
  const buttons = f.nodes().filter((node) => node.type === "button");
  for (const button of buttons) button.props.onClick();
  assert.deepEqual(destinations, [
    "/vault", "/notes", "/vault", "/notes", "/favorites", "/folders",
    "/recently-deleted", "/settings",
  ]);
  f.unmount();
});

test("sidebar links use exact routes, active styling, and close after selection", async () => {
  let closes = 0;
  const f = await mount("../src/app/components/NavigationSidebar.tsx", {
    isOpen: true,
    onClose() { closes++; },
    async onLockKeynest() {},
    profile: savedProfile,
  }, sidebarDependencies());
  const links = f.nodes().filter((node) => node.type === "NavLink");
  assert.deepEqual(links.map((link) => link.props.to), [
    "/home", "/vault", "/notes", "/favorites", "/folders", "/recently-deleted", "/settings",
  ]);
  for (const link of links) {
    assert.equal(link.props.end, true);
    assert.equal(link.props.className({ isActive: false }), "sidebar-link");
    assert.equal(link.props.className({ isActive: true }), "sidebar-link active");
    link.props.onClick();
  }
  assert.equal(closes, links.length);
  f.unmount();
});

test("sidebar retains disabled Soon items and closes before locking", async () => {
  let closes = 0, locks = 0;
  const f = await mount("../src/app/components/NavigationSidebar.tsx", {
    isOpen: true,
    onClose() { closes++; },
    async onLockKeynest() { locks++; },
    profile: savedProfile,
  }, sidebarDependencies());
  const buttons = f.nodes().filter((node) => node.type === "button");
  assert.equal(buttons.filter((button) => button.props.disabled).length, 1);
  const lock = buttons.find((button) => button.props.className.includes("sidebar-lock-button"));
  lock.props.onClick();
  await f.flush();
  assert.equal(closes, 1);
  assert.equal(locks, 1);
  f.unmount();
});

test("AppShell opens the sidebar and closes it from its callback, backdrop, or Escape", async () => {
  const f = await mount("../src/app/AppShell.tsx", {
    async onLockKeynest() {}, async onResetAuthenticated() {},
  }, appShellDependencies());
  const sidebar = () => f.find((node) => node.type === "NavigationSidebar");
  const titleBar = () => f.find((node) => node.type === "AppTitleBar");
  const backdrop = () => f.find((node) => node.props.className?.startsWith?.("sidebar-backdrop"));
  assert.equal(sidebar().props.isOpen, false);

  titleBar().props.onOpenNavigation(); await f.flush();
  assert.equal(sidebar().props.isOpen, true);
  sidebar().props.onClose(); await f.flush();
  assert.equal(sidebar().props.isOpen, false);

  titleBar().props.onOpenNavigation(); await f.flush();
  backdrop().props.onClick(); await f.flush();
  assert.equal(sidebar().props.isOpen, false);

  titleBar().props.onOpenNavigation(); await f.flush();
  f.listeners.get("keydown").forEach((listener) => listener({ key: "Escape" }));
  await f.flush();
  assert.equal(sidebar().props.isOpen, false);
  f.unmount();
});

test("AppShell preserves favorite toggling and profile/settings warnings", async () => {
  const f = await mount("../src/app/AppShell.tsx", {
    async onLockKeynest() {}, async onResetAuthenticated() {},
  }, appShellDependencies({
    profile: async () => { throw new Error("fixture failure"); },
    settings: { warning: "Settings fallback warning" },
  }));
  await f.flush();
  const banners = f.nodes().filter((node) => node.props.className === "settings-warning-banner");
  assert.deepEqual(banners.map((banner) => banner.props.children), [
    "Settings fallback warning",
    "KeyNest could not load the local profile. The default profile is shown.",
  ]);
  assert.equal(f.find((node) => node.type === "NavigationSidebar").props.profile, defaultProfile);
  const vaultRoute = f.nodes().find((node) => node.type === "Route" && node.props.path === "/vault");
  vaultRoute.props.element.props.onToggleFavorite("new-favorite");
  await f.flush();
  const updatedVault = f.nodes().find((node) => node.type === "Route" && node.props.path === "/vault").props.element;
  assert.deepEqual([...updatedVault.props.favoriteCredentialIds].sort(), ["new-favorite", "saved-favorite"]);
  const settingsRoute = f.nodes().find((node) => node.type === "Route" && node.props.path === "/settings");
  settingsRoute.props.element.props.onProfileSaved(savedProfile);
  await f.flush();
  assert.equal(f.find((node) => node.type === "NavigationSidebar").props.profile, savedProfile);
  const remainingBanners = f.nodes().filter((node) => node.props.className === "settings-warning-banner");
  assert.deepEqual(remainingBanners.map((banner) => banner.props.children), ["Settings fallback warning"]);
  f.unmount();
});
