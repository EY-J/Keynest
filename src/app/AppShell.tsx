import { useEffect, useState } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { useSettings } from "../features/settings/SettingsProvider";
import HostApprovalModal from "../features/autofill/components/HostApprovalModal";
import {
  loadFavoriteCredentialIds,
  saveFavoriteCredentialIds,
  toggleFavoriteCredentialId,
} from "../features/vault/credentialFavorites";
import HomePage from "../features/home/HomePage";
import FoldersPage from "../features/folders/FoldersPage";
import NotesPage from "../features/notes/NotesPage";
import RecentlyDeletedPage from "../features/recently-deleted/RecentlyDeletedPage";
import VaultPage from "../features/vault/VaultPage";
import SettingsPage from "../features/settings/SettingsPage";
import { profileClient } from "../features/profile/profileClient";
import {
  DEFAULT_PROFILE,
  type Profile,
} from "../features/profile/profileTypes";
import AppTitleBar from "./components/AppTitleBar";
import NavigationSidebar from "./components/NavigationSidebar";

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
        onClose={() => setIsSidebarOpen(false)}
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

      <Routes>
        <Route path="/" element={<Navigate to="/home" replace />} />
        <Route path="/home" element={<HomePage />} />
        <Route
          path="/vault"
          element={(
            <VaultPage
              favoriteCredentialIds={favoriteCredentialIds}
              favoritesOnly={false}
              onToggleFavorite={toggleFavorite}
            />
          )}
        />
        <Route path="/notes" element={<NotesPage />} />
        <Route path="/folders" element={<FoldersPage />} />
        <Route
          path="/favorites"
          element={(
            <VaultPage
              favoriteCredentialIds={favoriteCredentialIds}
              favoritesOnly={true}
              onToggleFavorite={toggleFavorite}
            />
          )}
        />
        <Route path="/recently-deleted" element={<RecentlyDeletedPage />} />
        <Route
          path="/settings"
          element={(
            <SettingsPage
              profile={profile}
              onProfileSaved={(savedProfile) => {
                setProfile(savedProfile);
                setProfileError("");
              }}
              onResetAuthenticated={onResetAuthenticated}
            />
          )}
        />
        <Route path="*" element={<Navigate to="/home" replace />} />
      </Routes>
      <HostApprovalModal />
    </div>
  );
}
