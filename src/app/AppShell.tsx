import { useEffect, useState } from "react";
import { useSettings } from "../features/settings/SettingsProvider";
import HostApprovalModal from "../features/autofill/components/HostApprovalModal";
import {
  loadFavoriteCredentialIds,
  saveFavoriteCredentialIds,
  toggleFavoriteCredentialId,
} from "../features/vault/credentialFavorites";
import HomePage from "../features/home/HomePage";
import VaultPage from "../features/vault/VaultPage";
import SettingsPage from "../features/settings/SettingsPage";
import { profileClient } from "../features/profile/profileClient";
import {
  DEFAULT_PROFILE,
  type Profile,
} from "../features/profile/profileTypes";
import AppTitleBar from "./components/AppTitleBar";
import NavigationSidebar from "./components/NavigationSidebar";

export type AuthenticatedDestination = "home" | "vault" | "favorites" | "settings";

type AppShellProps = {
  onLockKeynest: () => Promise<void>;
  onResetAuthenticated: (
    currentPassword: string,
    confirmation: "RESET KEYNEST",
  ) => Promise<void>;
};

export default function AppShell({
  onLockKeynest,
  onResetAuthenticated,
}: AppShellProps) {
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);
  const [profile, setProfile] = useState<Profile>(DEFAULT_PROFILE);
  const [profileError, setProfileError] = useState("");
  const [activeDestination, setActiveDestination] =
    useState<AuthenticatedDestination>("home");
  const [favoriteCredentialIds, setFavoriteCredentialIds] = useState<Set<string>>(
    loadFavoriteCredentialIds,
  );
  const { settings } = useSettings();

  useEffect(() => {
    let active = true;
    void profileClient.getProfile().then(
      (savedProfile) => {
        if (active) {
          setProfile(savedProfile);
          setProfileError("");
        }
      },
      () => {
        if (active) {
          setProfile(DEFAULT_PROFILE);
          setProfileError("KeyNest could not load the local profile. The default profile is shown.");
        }
      },
    );
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    saveFavoriteCredentialIds(favoriteCredentialIds);
  }, [favoriteCredentialIds]);

  useEffect(() => {
    if (!isSidebarOpen) {
      return;
    }

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setIsSidebarOpen(false);
      }
    }

    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [isSidebarOpen]);

  function navigate(destination: AuthenticatedDestination) {
    setActiveDestination(destination);
    setIsSidebarOpen(false);
  }

  function toggleFavorite(credentialId: string) {
    setFavoriteCredentialIds((current) =>
      toggleFavoriteCredentialId(current, credentialId),
    );
  }

  return (
    <div className="home-page">
      <AppTitleBar
        isNavigationOpen={isSidebarOpen}
        onOpenNavigation={() => setIsSidebarOpen((isOpen) => !isOpen)}
      />

      <NavigationSidebar
        isOpen={isSidebarOpen}
        activeDestination={activeDestination}
        onClose={() => setIsSidebarOpen(false)}
        onNavigate={navigate}
        onLockKeynest={onLockKeynest}
        profile={profile}
      />

      <button
        className={`sidebar-backdrop ${isSidebarOpen ? "visible" : ""}`}
        type="button"
        aria-label="Close navigation"
        tabIndex={isSidebarOpen ? 0 : -1}
        onClick={() => setIsSidebarOpen(false)}
      />

      {settings.warning ? (
        <p className="settings-warning-banner" role="status">
          {settings.warning}
        </p>
      ) : null}

      {profileError ? (
        <p className="settings-warning-banner" role="status">
          {profileError}
        </p>
      ) : null}

      {activeDestination === "home" ? (
        <HomePage onNavigateToVault={() => navigate("vault")} />
      ) : activeDestination === "vault" || activeDestination === "favorites" ? (
        <VaultPage
          favoriteCredentialIds={favoriteCredentialIds}
          favoritesOnly={activeDestination === "favorites"}
          onToggleFavorite={toggleFavorite}
        />
      ) : (
        <SettingsPage
          profile={profile}
          onProfileSaved={(savedProfile) => {
            setProfile(savedProfile);
            setProfileError("");
          }}
          onResetAuthenticated={onResetAuthenticated}
        />
      )}
      <HostApprovalModal />
    </div>
  );
}
